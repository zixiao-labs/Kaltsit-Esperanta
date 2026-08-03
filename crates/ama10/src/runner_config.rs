//! Org-scoped `runner-config.yaml` model aligned with Wuling DevOps autoscaler.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

pub const EMPTY_SEED: &str = r#"# runner-config.yaml — 组织级 Runner / Autoscaler 配置（GitOps）
# 保存后写入 {org}/config/config 仓库根目录。云凭证用 credentials_secret 引用
# 「机密」里的名称，不要写明文。

version: 1
default_tier: medium
idle_timeout: 5m

tiers:
  low:
    cpu: 2
    memory: 4Gi
    storage: 40Gi
  medium:
    cpu: 4
    memory: 8Gi
    storage: 80Gi
  high:
    cpu: 8
    memory: 16Gi
    storage: 160Gi

pools: []
"#;

pub const PROVIDER_ALIYUN: &str = "aliyun";
pub const PROVIDER_AWS: &str = "aws";
pub const PROVIDER_PROXMOX: &str = "proxmox";
pub const PROVIDER_VCENTER: &str = "vcenter";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerConfig {
    #[serde(default = "default_version")]
    pub version: i32,
    #[serde(default = "default_tier_name")]
    pub default_tier: String,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: String,
    #[serde(default)]
    pub tiers: BTreeMap<String, TierSpec>,
    #[serde(default)]
    pub pools: Vec<Pool>,
}

fn default_version() -> i32 {
    1
}

fn default_tier_name() -> String {
    "medium".into()
}

fn default_idle_timeout() -> String {
    "5m".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierSpec {
    #[serde(default)]
    pub cpu: i32,
    #[serde(default)]
    pub memory: String,
    #[serde(default)]
    pub storage: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    pub name: String,
    pub provider: String,
    pub tier: String,
    #[serde(default = "default_os")]
    pub os: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub min: i32,
    #[serde(default)]
    pub max: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliyun: Option<AliyunPool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws: Option<AwsPool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxmox: Option<ProxmoxPool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcenter: Option<VCenterPool>,
}

fn default_os() -> String {
    "linux".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AliyunPool {
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub zone_id: String,
    #[serde(default)]
    pub image_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instance_types: Vec<String>,
    #[serde(default)]
    pub vswitch_id: String,
    #[serde(default)]
    pub security_group_id: String,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub internet_max_bandwidth_out: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub internet_charge_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_disk_size: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_disk_category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_disk_performance_level: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data_disk_size: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data_disk_category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instance_charge_type: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spot: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password_secret: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub password_inherit: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key_pair_name: String,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub auto_release_hours: i32,
    #[serde(default)]
    pub credentials_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AwsPool {
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub ami: String,
    #[serde(default)]
    pub instance_type: String,
    #[serde(default)]
    pub subnet_id: String,
    #[serde(default)]
    pub security_group_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub iam_instance_profile: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spot: bool,
    #[serde(default)]
    pub credentials_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProxmoxPool {
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub node: String,
    #[serde(default)]
    pub template_vmid: i32,
    #[serde(default)]
    pub storage: String,
    #[serde(default)]
    pub bridge: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full_clone: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub insecure_tls: bool,
    #[serde(default)]
    pub credentials_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VCenterPool {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub datacenter: String,
    #[serde(default)]
    pub cluster: String,
    #[serde(default)]
    pub datastore: String,
    #[serde(default)]
    pub resource_pool: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub network: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub insecure_tls: bool,
    #[serde(default)]
    pub credentials_secret: String,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
}

impl RunnerConfig {
    pub fn parse(yaml: &str) -> Result<(Self, ValidationReport)> {
        let config: Self = serde_yaml::from_str(yaml).context("parse runner-config.yaml")?;
        let report = config.validate()?;
        Ok((config, report))
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("serialize runner-config.yaml")
    }

    pub fn default_seed() -> Self {
        Self::parse(EMPTY_SEED).expect("EMPTY_SEED must parse").0
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        let mut warnings = Vec::new();
        if self.version != 1 {
            warnings.push(format!(
                "unknown runner-config version {}; treating as version 1",
                self.version
            ));
        }
        if self.tiers.is_empty() {
            bail!("tiers must not be empty");
        }
        if !self.tiers.contains_key(&self.default_tier) {
            bail!(
                "default_tier {:?} is not defined in tiers",
                self.default_tier
            );
        }
        if parse_go_duration(&self.idle_timeout).is_err() {
            bail!("invalid idle_timeout {:?}", self.idle_timeout);
        }

        let mut names = BTreeSet::new();
        for pool in &self.pools {
            if pool.name.trim().is_empty() {
                bail!("pool name is required");
            }
            if !names.insert(pool.name.clone()) {
                bail!("duplicate pool name {:?}", pool.name);
            }
            if !self.tiers.contains_key(&pool.tier) {
                bail!(
                    "pool {:?}: tier {:?} is not defined in tiers",
                    pool.name,
                    pool.tier
                );
            }
            if pool.min < 0 || pool.max < 0 || pool.min > pool.max {
                bail!(
                    "pool {:?}: require 0 <= min <= max (got min={}, max={})",
                    pool.name,
                    pool.min,
                    pool.max
                );
            }
            match pool.os.as_str() {
                "linux" | "windows" => {}
                "macos" => bail!(
                    "pool {:?}: macos cannot be autoscaled; register static runners instead",
                    pool.name
                ),
                other => bail!("pool {:?}: unsupported os {:?}", pool.name, other),
            }

            let provider_blocks = [
                (PROVIDER_ALIYUN, pool.aliyun.is_some()),
                (PROVIDER_AWS, pool.aws.is_some()),
                (PROVIDER_PROXMOX, pool.proxmox.is_some()),
                (PROVIDER_VCENTER, pool.vcenter.is_some()),
            ];
            let present: Vec<_> = provider_blocks
                .iter()
                .filter(|(_, present)| *present)
                .map(|(name, _)| *name)
                .collect();
            if present.len() != 1 {
                bail!(
                    "pool {:?}: exactly one of aliyun/aws/proxmox/vcenter must be set",
                    pool.name
                );
            }
            if present[0] != pool.provider.as_str() {
                bail!(
                    "pool {:?}: provider {:?} does not match block {:?}",
                    pool.name,
                    pool.provider,
                    present[0]
                );
            }

            match pool.provider.as_str() {
                PROVIDER_ALIYUN => {
                    let block = pool.aliyun.as_ref().expect("checked");
                    if block.credentials_secret.trim().is_empty() {
                        bail!(
                            "pool {:?}: aliyun.credentials_secret is required",
                            pool.name
                        );
                    }
                    if block.instance_type.is_none() && block.instance_types.is_empty() {
                        bail!(
                            "pool {:?}: set aliyun.instance_type or aliyun.instance_types",
                            pool.name
                        );
                    }
                    if pool.os == "windows"
                        && block.password_secret.is_empty()
                        && !block.password_inherit
                    {
                        bail!(
                            "pool {:?}: windows aliyun pools need password_secret or password_inherit",
                            pool.name
                        );
                    }
                }
                PROVIDER_AWS => {
                    let block = pool.aws.as_ref().expect("checked");
                    if block.credentials_secret.trim().is_empty() {
                        bail!("pool {:?}: aws.credentials_secret is required", pool.name);
                    }
                    if block.ami.trim().is_empty() || block.instance_type.trim().is_empty() {
                        bail!(
                            "pool {:?}: aws.ami and aws.instance_type are required",
                            pool.name
                        );
                    }
                }
                PROVIDER_PROXMOX | PROVIDER_VCENTER => {
                    warnings.push(format!(
                        "pool {:?}: provider {:?} is accepted but VM provisioning is not implemented yet",
                        pool.name, pool.provider
                    ));
                    let secret = match pool.provider.as_str() {
                        PROVIDER_PROXMOX => {
                            &pool.proxmox.as_ref().expect("checked").credentials_secret
                        }
                        _ => &pool.vcenter.as_ref().expect("checked").credentials_secret,
                    };
                    if secret.trim().is_empty() {
                        bail!(
                            "pool {:?}: {}.credentials_secret is required",
                            pool.name,
                            pool.provider
                        );
                    }
                }
                other => bail!("pool {:?}: unsupported provider {:?}", pool.name, other),
            }
        }

        Ok(ValidationReport { warnings })
    }
}

fn parse_go_duration(raw: &str) -> Result<std::time::Duration> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("empty duration");
    }
    // Accept Go-style concatenated units: 1h30m, 5m, 30s.
    let mut total = std::time::Duration::ZERO;
    let mut number = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        if number.is_empty() {
            bail!("invalid duration {raw:?}");
        }
        let value: u64 = number.parse().context("duration number")?;
        number.clear();
        let piece = match ch {
            'h' => std::time::Duration::from_secs(value * 3600),
            'm' => std::time::Duration::from_secs(value * 60),
            's' => std::time::Duration::from_secs(value),
            _ => bail!("unsupported duration unit {ch:?} in {raw:?}"),
        };
        total += piece;
    }
    if !number.is_empty() {
        bail!("duration {raw:?} missing unit");
    }
    if total.is_zero() {
        bail!("duration must be > 0");
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_seed() {
        let (config, report) = RunnerConfig::parse(EMPTY_SEED).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.default_tier, "medium");
        assert_eq!(config.tiers.len(), 3);
        assert!(config.pools.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn parses_aliyun_pool() {
        let yaml = r#"
version: 1
default_tier: medium
idle_timeout: 5m
tiers:
  medium: { cpu: 4, memory: 8Gi, storage: 80Gi }
pools:
  - name: aliyun-medium
    provider: aliyun
    tier: medium
    labels: [linux, docker]
    min: 0
    max: 5
    aliyun:
      region: cn-hangzhou
      zone_id: cn-hangzhou-i
      image_id: m-xxxxxxxx
      instance_type: ecs.g7.large
      vswitch_id: vsw-xxxxxxxx
      security_group_id: sg-xxxxxxxx
      credentials_secret: ALIYUN_CREDS
"#;
        let (config, _) = RunnerConfig::parse(yaml).unwrap();
        assert_eq!(config.pools.len(), 1);
        assert_eq!(config.pools[0].provider, PROVIDER_ALIYUN);
        let roundtrip = config.to_yaml().unwrap();
        let (again, _) = RunnerConfig::parse(&roundtrip).unwrap();
        assert_eq!(again.pools[0].name, "aliyun-medium");
    }

    #[test]
    fn rejects_macos_pool() {
        let yaml = r#"
version: 1
default_tier: medium
idle_timeout: 5m
tiers:
  medium: { cpu: 4, memory: 8Gi, storage: 80Gi }
pools:
  - name: mac
    provider: aws
    tier: medium
    os: macos
    max: 1
    aws:
      region: us-west-2
      ami: ami-x
      instance_type: mac2.metal
      subnet_id: subnet-x
      security_group_ids: [sg-x]
      credentials_secret: AWS_CREDS
"#;
        assert!(RunnerConfig::parse(yaml).is_err());
    }
}
