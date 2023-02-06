// Copyright 2023 The Amphitheatre Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::fmt::Display;

use convert_case::{Case, Casing};
use k8s_openapi::api::core::v1::{ContainerPort, EnvVar, ServicePort};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono::Utc;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::url;
use crate::resources::to_env_var;

#[derive(
    CustomResource, Default, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate, PartialEq,
)]
#[kube(
    group = "amphitheatre.app",
    version = "v1",
    kind = "Actor",
    status = "ActorStatus",
    namespaced
)]
pub struct ActorSpec {
    /// The name of the actor.
    pub name: String,
    /// The description of the actor.
    pub description: String,
    /// Specifies the image to launch the container. The image must follow
    /// the Open Container Specification addressable image format.
    /// such as: [<registry>/][<project>/]<image>[:<tag>|@<digest>].
    pub image: String,
    /// overrides the default command declared by the container image
    /// (i.e. by Dockerfile’s CMD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Source code repository the package should be cloned from.
    /// e.g. https://github.com/amphitheatre-app/amphitheatre.git.
    pub repository: String,
    /// Relative path from the repo root to the configuration file.
    /// eg. getting-started/.amp.toml. default is `./.amp.toml`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Git ref the package should be cloned from. eg. master or main
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// The selected commit of the actor.
    pub commit: String,
    /// Defines environment variables set in the container. Any boolean values:
    /// true, false, yes, no, SHOULD be enclosed in quotes to ensure they are
    /// not converted to True or False by the YAML parser.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environments: Option<HashMap<String, String>>,
    /// Depend on other partners from other repositories, or subdirectories on
    /// your local file system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partners: Option<Vec<Partner>>,
    /// Defines the behavior of a service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
    /// sync mode, if enabled, pulls the latest code from source version
    /// control in real time via Webhook, etc. and then rebuilds and deploys it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<bool>,
    /// Describes how images are built.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
}

impl Actor {
    // For kpack Image name
    #[inline]
    pub fn build_name(&self) -> String {
        format!("{}-{}", self.spec.name, self.spec.commit)
    }

    pub fn docker_tag(&self) -> String {
        format!("{}:{}", self.spec.image, self.spec.commit)
    }
}

impl ActorSpec {
    #[inline]
    pub fn url(&self) -> String {
        url(&self.repository, &self.reference, &self.path)
    }

    pub fn environments(&self) -> Option<Vec<EnvVar>> {
        if let Some(vars) = &self.environments {
            return Some(to_env_var(vars));
        }

        None
    }

    pub fn container_ports(&self) -> Option<Vec<ContainerPort>> {
        let services = self.services.as_ref()?;
        let mut ports: Vec<ContainerPort> = vec![];

        for service in services {
            let mut items = service
                .ports
                .iter()
                .map(|p| ContainerPort {
                    container_port: p.port,
                    protocol: p.protocol.clone(),
                    ..Default::default()
                })
                .collect();
            ports.append(&mut items);
        }

        Some(ports)
    }

    pub fn service_ports(&self) -> Option<Vec<ServicePort>> {
        let services = self.services.as_ref()?;
        let mut ports: Vec<ServicePort> = vec![];

        for service in services {
            let mut items = service
                .ports
                .iter()
                .filter(|p| p.expose.unwrap_or_default())
                .map(|p| ServicePort {
                    port: p.port,
                    protocol: p.protocol.clone(),
                    ..Default::default()
                })
                .collect();
            ports.append(&mut items);
        }

        if ports.is_empty() {
            None
        } else {
            Some(ports)
        }
    }

    #[inline]
    pub fn has_dockerfile(&self) -> bool {
        self.build.is_some() && self.build.as_ref().unwrap().dockerfile.is_some()
    }
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, Eq, Hash, PartialEq)]
pub struct Partner {
    /// The name of the character.
    pub name: String,
    /// Source code repository the package should be cloned from.
    /// e.g. https://github.com/amphitheatre-app/amphitheatre.git.
    pub repository: String,
    /// Relative path from the repo root to the configuration file.
    /// eg. getting-started/amp.toml. default is `./.amp.toml`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Git ref the package should be cloned from. eg. master or main
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl Partner {
    #[inline]
    pub fn url(&self) -> String {
        url(&self.repository, &self.reference, &self.path)
    }
}

/// Defines the behavior of a service
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct Service {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub ports: Vec<Port>,
}

/// List of ports to expose from the container.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct Port {
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expose: Option<bool>,
}

/// Describes how images are built.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct Build {
    /// Global parameters
    ///
    /// Directory containing the artifact's sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Environment variables, in the key=value form, passed to the build.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Builds images using kaniko.
    ///
    /// Locates the Dockerfile relative to workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,

    /// Builds images using Cloud Native Buildpacks.
    ///
    /// Builder image used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<String>,
    /// A list of strings, where each string is a specific buildpack to use with the builder.
    /// If you specify buildpacks the builder image automatic detection will be ignored.
    /// These buildpacks will be used to build the Image from your source code. Order matters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buildpacks: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct ActorStatus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conditions: Vec<Condition>,
}

impl ActorStatus {
    pub fn pending(&self) -> bool {
        self.state(ActorState::Pending, true)
    }

    pub fn building(&self) -> bool {
        self.state(ActorState::Building, true)
    }

    pub fn running(&self) -> bool {
        self.state(ActorState::Running, true)
    }

    pub fn failed(&self) -> bool {
        self.state(ActorState::Failed, true)
    }

    fn state(&self, s: ActorState, status: bool) -> bool {
        self.conditions.iter().any(|condition| {
            condition.type_ == s.to_string()
                && condition.status == status.to_string().to_case(Case::Pascal)
        })
    }
}

pub enum ActorState {
    Pending,
    Building,
    Running,
    Failed,
}

impl ActorState {
    pub fn pending() -> Condition {
        ActorState::create(ActorState::Pending, true, "Created", None)
    }

    pub fn building() -> Condition {
        ActorState::create(ActorState::Building, true, "Build", None)
    }

    pub fn running(status: bool, reason: &str, message: Option<String>) -> Condition {
        ActorState::create(ActorState::Running, status, reason, message)
    }

    pub fn failed(status: bool, reason: &str, message: Option<String>) -> Condition {
        ActorState::create(ActorState::Failed, status, reason, message)
    }

    #[inline]
    fn create(state: ActorState, status: bool, reason: &str, message: Option<String>) -> Condition {
        Condition {
            type_: state.to_string(),
            status: status.to_string().to_case(Case::Pascal),
            last_transition_time: Time(Utc::now()),
            reason: reason.to_case(Case::Pascal),
            observed_generation: None,
            message: match message {
                Some(message) => message,
                None => "".to_string(),
            },
        }
    }
}

impl Display for ActorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorState::Pending => f.write_str("Pending"),
            ActorState::Building => f.write_str("Building"),
            ActorState::Running => f.write_str("Running"),
            ActorState::Failed => f.write_str("Failed"),
        }
    }
}
