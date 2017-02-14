// Copyright 2017 VMware, Inc. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use rabble::{self, Process, Pid, CorrelationId, Envelope};
use funfsm::Fsm;
use msg::Msg;
use super::vr_fsm::VrTypes;
use super::vr_envelope::VrEnvelope;
use super::vrmsg::VrMsg;
use super::vr_ctx_summary::VrCtxSummary;
use super::super::admin::{AdminReq, AdminRpy};

/// A replica wraps a VR FSM as a process so that it can receive messsages from inside rabble
/// It also takes care to only pass messages of type VrEnvelope to the FSM.
pub struct Replica {
    pid: Pid,
    fsm: Fsm<VrTypes>,
    output: Vec<Envelope<Msg>>
}

impl Replica {
    pub fn new(pid: Pid, fsm: Fsm<VrTypes>) -> Replica {
        Replica {
            pid: pid,
            fsm: fsm,
            output: Vec::with_capacity(1)
        }
    }
}

impl Process for Replica {
    type Msg = Msg;

    fn init(&mut self, from: Pid) -> Vec<Envelope<Msg>> {
        let c_id = CorrelationId::pid(self.pid.clone());
        let envelope = VrEnvelope::new(self.pid.clone(), from, VrMsg::Tick, c_id);
        // Convert Vec<FsmOuput> to Vec<Envelope<Msg>>
        self.fsm.send(envelope).into_iter().map(|e| e.into()).collect()
    }

    fn handle(&mut self,
              msg: rabble::Msg<Msg>,
              from: Pid,
              correlation_id: Option<CorrelationId>) -> &mut Vec<Envelope<Msg>>
    {
        let correlation_id = correlation_id.map_or(CorrelationId::pid(self.pid.clone()),
                                                   |c_id| c_id);
        match msg {
            rabble::Msg::User(Msg::AdminReq(AdminReq::GetReplicaState(_))) => {
                let (state, ctx) = self.fsm.get_state();
                let summary = VrCtxSummary::new(state, ctx);
                let rpy = AdminRpy::ReplicaState(summary);
                let msg = rabble::Msg::User(Msg::AdminRpy(rpy));
                let envelope = Envelope::new(from, self.pid.clone(), msg, Some(correlation_id));
                self.output.push(envelope);
            },
            rabble::Msg::User(Msg::Vr(vrmsg)) => {
               let vr_envelope = VrEnvelope::new(self.pid.clone(), from, vrmsg, correlation_id);
               self.output.extend(self.fsm.send(vr_envelope).into_iter().map(|e| e.into()));
            },
            _ => {
                let msg = rabble::Msg::User(Msg::Error("Invalid Msg Received".to_string()));
                let envelope = Envelope::new(from, self.pid.clone(), msg, Some(correlation_id));
                self.output.push(envelope);
            }
        }
        &mut self.output
    }

}


/// When we create a namesapce, the initial group of replicas is at epoch 1. When a reconfiguration
/// occurs we bump the epoch so when we gossip the information around to start the fsms, we can
/// chose the latest version.
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct VersionedReplicas {
    pub epoch: u64,
    pub op: u64,
    pub replicas: Vec<Pid>
}

impl VersionedReplicas {
    pub fn new() -> VersionedReplicas {
        VersionedReplicas {
            epoch: 0,
            op: 0,
            replicas: Vec::new()
        }
    }
}
