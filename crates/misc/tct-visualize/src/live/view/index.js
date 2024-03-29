const liveViewSettings = {
    animationDuration: 200,
    animationEasing: d3.easeCubic,
    renderInterval: 250,
    pollRetry: 1000,
    longPollDelay: 0,
    shortPollInterval: 500,
    initialPrecision: 1,
    targetMaxRenderTime: 250,
    precisionAdjustFactor: 1.5,
};

function transition() {
    return d3.transition("main")
        .ease(liveViewSettings.animationEasing)
        .duration(liveViewSettings.animationDuration);
}

const graphviz = d3.select("#graph").graphviz()
    .transition(transition)
    .growEnteringEdges(false) // d3-graphviz bug: if enabled, this causes an error
    .tweenShapes(false) // Increases performance
    .logEvents(false) // Disabling logging increases performance
    // Set the SVG to fill the window
    .width(window.innerWidth)
    .height(window.innerHeight)
    .fit(true)
     // Start the event loop once the graphviz stuff is loaded
    .on("initEnd", run);

function run() {
    // When the window is resized, resize the graphviz SVG also
    window.addEventListener("resize", () => {
        // Immediately resize it
        d3.select("#graph > svg")
            .attr("width", window.innerWidth)
            .attr("height", window.innerHeight);
        // Resize it in all future renders
        graphviz
            .width(window.innerWidth)
            .height(window.innerHeight);
    });

    // Initial state
    let retrying = false;
    let changed = false;
    let lastChanged = performance.now();
    let renderedRecently = false;
    let precision = liveViewSettings.initialPrecision;
    let latestDot = 'digraph {}';
    let forgotten = 0;
    let position = {
        epoch: 0,
        block: 0,
        commitment: 0,
    };

    // Long-poll loop to get the latest dot render of the tree
    function poll(long, graph) {
        if (graph === undefined) {
            graph = true;
        }

        let queryString = "";
        if (position !== null) {
            queryString = "?epoch=" + position.epoch + "&block=" + position.block + "&commitment=" + position.commitment + "&forgotten=" + forgotten + "&next=" + long + "&graph=" + graph;
        } else {
            queryString = "?full=true&forgotten=" + forgotten + "&next=" + long + "&graph=" + graph;
        }
        let url = window.location.href + "/dot" + queryString;

        d3.json(url).then(response => {
            // Only update the graph if we asked for it (otherwise it'll be null)
            if (!graph) {
                // If changes have happened but we didn't get the latest graph, go fetch it now
                if (response.forgotten !== forgotten || response.position !== position) {
                    poll(long, graph);
                    return;
                } else {
                    // If no changes have happened and we don't have a new graph, stop
                    return;
                }
            }

            // Double check that the graph has really changed
            if (response.forgotten !== forgotten || response.position !== position || response.graph !== latestDot) {
                latestDot = response.graph;
                changed = true;
                lastChanged = performance.now();

                // We've gotten the graph, so update our state and trigger a render
                forgotten = response.forgotten;
                position = response.position;
                // Start a new render task, if one isn't already in progress
                setTimeout(render, 0);
            }

            // Schedule the polling to recur
            if (long) {
                setTimeout(() => poll(long, graph), liveViewSettings.longPollDelay);
            }
        }).catch(error => {
            console.log(error);
            retry();
        });
    }

    // Retry polling until we get a response, then reload the page
    function retry() {
        window.location.reload(); // Quick and dirty: don't bother retrying, just kill the page by reloading it

        if (!retrying) {
            retrying = true;
            d3.text(window.location.href).then(() => {
                window.location.reload();
            }).catch(() => {
                setTimeout(retry, liveViewSettings.pollRetry);
            });
        }
    }

    // Render the current dot, if it has changed, and continue rendering until it hasn't changed
    // from underneath us while rendering
    function render() {
        if (changed && !renderedRecently) {
            // Mark the render as having started, so other calls to render will stop if there
            // haven't been other updates
            changed = false;
            // Mark the render as being recent, and schedule the recency to expire after the
            // recency duration
            renderedRecently = true;
            setTimeout(() => {
                // After the recency duration, mark the render as no longer being recent, and
                // re-render if anything has changed
                renderedRecently = false;
                render();
            }, liveViewSettings.renderInterval);
            // Render the graph
            let start = performance.now();
            graphviz.tweenPrecision(precision + "%").dot(latestDot, () => {
                // If the pre-calculation took too long, decrease the tweening precision
                let end = performance.now();
                let elapsed = end - start;
                if (elapsed > liveViewSettings.targetMaxRenderTime) {
                    // Increase the precision percentage if rendering took too long
                    precision = precision * liveViewSettings.precisionAdjustFactor;
                } else if (elapsed < liveViewSettings.targetMaxRenderTime * liveViewSettings.precisionAdjustFactor) {
                    // Decrease the precision percentage if rendering was really fast
                    precision = precision / liveViewSettings.precisionAdjustFactor;
                }
                // Round precision to nearest integer
                precision = Math.round(precision);
                // Don't let precision go above 100%
                precision = Math.min(precision, 100);
                // Don't let precision go below 1%
                precision = Math.max(precision, 1);
            }).render(render);
        }
    }

    // Initially do a short-poll to set up the first state
    poll(true);

    // Start the long-poll loop over non-interior changes
    poll(false);

    // Check for changes periodically by doing a short poll without immediate request for the graph:
    // if it discovers the position has changed, it will initiate another poll where it does a real
    // update by pulling the graph
    setInterval(() => {
        let sinceLastChange = performance.now() - lastChanged;
        if (sinceLastChange >= liveViewSettings.shortPollInterval) {
            poll(false, false);
        }
    }, liveViewSettings.shortPollInterval);

    // Interior mutation caused by evaluating the lazy frontier hashes won't cause the position or
    // forgotten index to advance, so it won't be caught by the long-poll loop: we use the SSE
    // endpoint to monitor for these changes, and trigger an immediate short poll when they occur.
    // Additionally, if we detect that the tree has gone "backwards in time" either via forgotten
    // count or position, we trigger a reload of the page, because the page state doesn't match the
    // tree state, so rather than ensuring we set up all the mutable state correctly again, the
    // easiest thing to do is start fresh.
    let changes = new EventSource(window.location.href + "/extra-changes");
    changes.addEventListener("changed", (event) => {
        // When a change occurs, check to see if *nothing has changed* about the position and
        // forgotten count: only then, do a short poll to get the latest dot.
        let response = JSON.parse(event.data);
        // If forgotten went strictly backwards or position went strictly backwards, refresh the
        // page, because this means that the tree has been reset since we last saw it, and our state
        // doesn't correctly reflect reality, so long polling won't work
        let forgottenBackwards = response.forgotten < forgotten;
        let positionBackwards =
            // Both positions are non-null (i.e. tree went from a non-full state to a non-full state)
            response.position !== null && position !== null
            && (response.position.epoch < position.epoch // epoch went back, or...
                || (response.position.epoch === position.epoch // epoch stayed the same, and...
                    && response.position.block < position.block) // block went back, or...
                || (response.position.epoch === position.epoch // epoch stayed the same, and...
                    && response.position.block === position.block // block stayed the same, and...
                    && response.position.commitment < position.commitment)); // commitment went back
        if (forgottenBackwards || positionBackwards) {
            location.reload();
        }
        // Figure out whether the event was an interior mutation, and only do a short-poll if it was
        // an interior mutation (otherwise we'd be wasting our time because the long poll will get
        // to that change)
        let forgottenSame = response.forgotten === forgotten;
        let positionSame =
            (response.position === null && position === null)
            || (response.position !== null && position !== null
                && response.position.epoch === position.epoch
                && response.position.block === position.block
                && response.position.commitment === position.commitment)
        if (forgottenSame && positionSame) {
            poll(false);
        }
    });
}