use anyhow::Result;
use async_trait::async_trait;
use ibc_types::core::{
    ics03_connection::connection::{ConnectionEnd, State as ConnectionState},
    ics04_channel::{
        channel::{ChannelEnd, Counterparty, State as ChannelState},
        msgs::chan_open_try::MsgChannelOpenTry,
    },
    ics24_host::identifier::PortId,
};
use penumbra_storage::{StateRead, StateWrite};

use crate::{
    component::{
        app_handler::{AppHandlerCheck, AppHandlerExecute},
        channel::{stateful::proof_verification::ChannelProofVerifier, StateWriteExt},
        connection::StateReadExt,
        transfer::Ics20Transfer,
        MsgHandler,
    },
    event,
};

#[async_trait]
impl MsgHandler for MsgChannelOpenTry {
    async fn check_stateless(&self) -> Result<()> {
        connection_hops_eq_1(self)?;

        Ok(())
    }

    async fn try_execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        tracing::debug!(msg = ?self);
        let connection_on_b = verify_connections_open(&state, self).await?;

        // TODO: do we want to do capability authentication?
        // TODO: version intersection

        let expected_channel_on_a = ChannelEnd {
            state: ChannelState::Init,
            ordering: self.ordering,
            remote: Counterparty::new(self.port_id_on_b.clone(), None),
            connection_hops: vec![connection_on_b
                .counterparty()
                .connection_id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no counterparty connection id provided"))?],
            version: self.version_supported_on_a.clone(),
        };

        tracing::debug!(?self, ?expected_channel_on_a);

        state
            .verify_channel_proof(
                &connection_on_b,
                &self.proof_chan_end_on_a,
                &self.proof_height_on_a,
                &self.chan_id_on_a,
                &self.port_id_on_a,
                &expected_channel_on_a,
            )
            .await?;

        let transfer = PortId::transfer();
        if self.port_id_on_b == transfer {
            Ics20Transfer::chan_open_try_check(&mut state, self).await?;
        } else {
            return Err(anyhow::anyhow!("invalid port id"));
        }

        let channel_id = state.next_channel_id().await.unwrap();
        let new_channel = ChannelEnd {
            state: ChannelState::TryOpen,
            ordering: self.ordering,
            remote: Counterparty::new(self.port_id_on_a.clone(), Some(self.chan_id_on_a.clone())),
            connection_hops: self.connection_hops_on_b.clone(),
            version: self.version_supported_on_a.clone(),
        };

        state.put_channel(&channel_id, &self.port_id_on_b, new_channel.clone());
        state.put_send_sequence(&channel_id, &self.port_id_on_b, 1);
        state.put_recv_sequence(&channel_id, &self.port_id_on_b, 1);
        state.put_ack_sequence(&channel_id, &self.port_id_on_b, 1);

        state.record(event::channel_open_try(
            &self.port_id_on_b,
            &channel_id,
            &new_channel,
        ));

        let transfer = PortId::transfer();
        if self.port_id_on_b == transfer {
            Ics20Transfer::chan_open_try_execute(state, self).await;
        } else {
            return Err(anyhow::anyhow!("invalid port id"));
        }

        Ok(())
    }
}

pub fn connection_hops_eq_1(msg: &MsgChannelOpenTry) -> anyhow::Result<()> {
    if msg.connection_hops_on_b.len() != 1 {
        return Err(anyhow::anyhow!(
            "currently only channels with one connection hop are supported"
        ));
    }
    Ok(())
}

async fn verify_connections_open<S: StateRead>(
    state: S,
    msg: &MsgChannelOpenTry,
) -> anyhow::Result<ConnectionEnd> {
    let connection = state
        .get_connection(&msg.connection_hops_on_b[0])
        .await?
        .ok_or_else(|| anyhow::anyhow!("connection not found"))?;

    if connection.state != ConnectionState::Open {
        Err(anyhow::anyhow!("connection for channel is not open"))
    } else {
        Ok(connection)
    }
}