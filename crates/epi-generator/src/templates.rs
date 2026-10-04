use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrgTemplate {
    MicroBusiness,
    SmallBusiness,
    MediumEnterprise,
    LargeEnterprise,
    GovernmentAgency,
    CloudNativeStartup,
    HybridEnterprise,
    SoftwareCompany,
    University,
    ManagedServiceProvider,
}

pub const TEMPLATE_IDS: [&str; 10] = [
    "micro-business",
    "small-business",
    "medium-enterprise",
    "large-enterprise",
    "government-agency",
    "cloud-native-startup",
    "hybrid-enterprise",
    "software-company",
    "university",
    "managed-service-provider",
];

impl OrgTemplate {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "micro-business" | "micro" => Some(Self::MicroBusiness),
            "small-business" | "small" => Some(Self::SmallBusiness),
            "medium-enterprise" | "medium" => Some(Self::MediumEnterprise),
            "large-enterprise" | "large" => Some(Self::LargeEnterprise),
            "government-agency" | "government" => Some(Self::GovernmentAgency),
            "cloud-native-startup" | "cloud-native" => Some(Self::CloudNativeStartup),
            "hybrid-enterprise" | "hybrid" => Some(Self::HybridEnterprise),
            "software-company" | "software" => Some(Self::SoftwareCompany),
            "university" => Some(Self::University),
            "managed-service-provider" | "msp" => Some(Self::ManagedServiceProvider),
            _ => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::MicroBusiness => "micro-business",
            Self::SmallBusiness => "small-business",
            Self::MediumEnterprise => "medium-enterprise",
            Self::LargeEnterprise => "large-enterprise",
            Self::GovernmentAgency => "government-agency",
            Self::CloudNativeStartup => "cloud-native-startup",
            Self::HybridEnterprise => "hybrid-enterprise",
            Self::SoftwareCompany => "software-company",
            Self::University => "university",
            Self::ManagedServiceProvider => "managed-service-provider",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::MicroBusiness => "Micro business",
            Self::SmallBusiness => "Small business",
            Self::MediumEnterprise => "Medium enterprise",
            Self::LargeEnterprise => "Large enterprise",
            Self::GovernmentAgency => "Government agency",
            Self::CloudNativeStartup => "Cloud-native startup",
            Self::HybridEnterprise => "Hybrid enterprise",
            Self::SoftwareCompany => "Software company",
            Self::University => "University",
            Self::ManagedServiceProvider => "Managed service provider",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TemplateProfile {
    pub maturity: f64,
    pub user_share: f64,
    pub endpoint_share: f64,
    pub server_share: f64,
    pub admin_per_thousand: f64,
    pub service_per_thousand: f64,
    pub saas_per_thousand: f64,
    pub repo_per_thousand: f64,
    pub pipeline_per_thousand: f64,
    pub namespace_per_thousand: f64,
    pub workload_per_thousand: f64,
    pub segments: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Quotas {
    pub users: usize,
    pub endpoints: usize,
    pub servers: usize,
    pub admins: usize,
    pub service_accounts: usize,
    pub saas: usize,
    pub repos: usize,
    pub pipelines: usize,
    pub namespaces: usize,
    pub workloads: usize,
}

impl TemplateProfile {
    /// `budget` is the number of ordinary entities, excluding named hubs and
    /// segment routers. The returned quotas sum to `budget`.
    pub fn quotas(self, budget: usize) -> Quotas {
        if budget == 0 {
            return Quotas::default();
        }
        let scale = (budget as f64 / 1_000.0).max(0.02);
        let mut quotas = Quotas {
            admins: ((self.admin_per_thousand * scale).round() as usize).clamp(1, budget / 5 + 1),
            service_accounts: ((self.service_per_thousand * scale).round() as usize).max(1),
            saas: ((self.saas_per_thousand * scale).round() as usize).max(1),
            repos: ((self.repo_per_thousand * scale).round() as usize).max(1),
            pipelines: ((self.pipeline_per_thousand * scale).round() as usize).max(1),
            namespaces: ((self.namespace_per_thousand * scale).round() as usize).max(1),
            workloads: ((self.workload_per_thousand * scale).round() as usize).max(1),
            users: 0,
            endpoints: 0,
            servers: 0,
        };
        let mut fixed = quotas.admins
            + quotas.service_accounts
            + quotas.saas
            + quotas.repos
            + quotas.pipelines
            + quotas.namespaces
            + quotas.workloads;
        while fixed > budget / 2
            && (quotas.workloads > 1 || quotas.saas > 1 || quotas.service_accounts > 1)
        {
            if quotas.workloads > 1 {
                quotas.workloads -= 1;
            } else if quotas.saas > 1 {
                quotas.saas -= 1;
            } else if quotas.service_accounts > 1 {
                quotas.service_accounts -= 1;
            }
            fixed = quotas.admins
                + quotas.service_accounts
                + quotas.saas
                + quotas.repos
                + quotas.pipelines
                + quotas.namespaces
                + quotas.workloads;
        }
        let pool = budget.saturating_sub(fixed);
        let share = (self.user_share + self.endpoint_share + self.server_share).max(0.01);
        quotas.users = ((pool as f64) * (self.user_share / share)).floor() as usize;
        quotas.endpoints = ((pool as f64) * (self.endpoint_share / share)).floor() as usize;
        let used = fixed + quotas.users + quotas.endpoints;
        quotas.servers = budget.saturating_sub(used);
        let sum = quota_total(&quotas);
        if sum < budget {
            quotas.users += budget - sum;
        }
        while quota_total(&quotas) > budget {
            if quotas.users > 0 {
                quotas.users -= 1;
            } else if quotas.endpoints > 0 {
                quotas.endpoints -= 1;
            } else if quotas.servers > 0 {
                quotas.servers -= 1;
            } else if quotas.workloads > 0 {
                quotas.workloads -= 1;
            } else if quotas.saas > 0 {
                quotas.saas -= 1;
            } else if quotas.service_accounts > 0 {
                quotas.service_accounts -= 1;
            } else if quotas.admins > 0 {
                quotas.admins -= 1;
            } else if quotas.repos > 0 {
                quotas.repos -= 1;
            } else if quotas.pipelines > 0 {
                quotas.pipelines -= 1;
            } else if quotas.namespaces > 0 {
                quotas.namespaces -= 1;
            } else {
                break;
            }
        }
        quotas
    }
}

fn quota_total(quotas: &Quotas) -> usize {
    quotas.admins
        + quotas.service_accounts
        + quotas.saas
        + quotas.repos
        + quotas.pipelines
        + quotas.namespaces
        + quotas.workloads
        + quotas.users
        + quotas.endpoints
        + quotas.servers
}

pub fn template(kind: OrgTemplate) -> TemplateProfile {
    match kind {
        OrgTemplate::MicroBusiness => profile(0.28, 0.55, 0.38, 0.07, 8, 6, 12, 2, 1, 1, 2, 2),
        OrgTemplate::SmallBusiness => profile(0.36, 0.52, 0.40, 0.08, 10, 8, 16, 3, 2, 1, 3, 3),
        OrgTemplate::MediumEnterprise => profile(0.48, 0.50, 0.40, 0.10, 12, 14, 18, 6, 4, 3, 8, 6),
        OrgTemplate::LargeEnterprise => {
            profile(0.58, 0.48, 0.40, 0.12, 14, 18, 20, 8, 5, 4, 10, 12)
        }
        OrgTemplate::GovernmentAgency => profile(0.52, 0.62, 0.30, 0.08, 10, 12, 14, 4, 2, 2, 4, 8),
        OrgTemplate::CloudNativeStartup => {
            profile(0.62, 0.35, 0.15, 0.05, 18, 30, 22, 20, 12, 8, 40, 4)
        }
        OrgTemplate::HybridEnterprise => profile(0.5, 0.46, 0.36, 0.10, 12, 16, 18, 8, 5, 4, 12, 8),
        OrgTemplate::SoftwareCompany => {
            profile(0.57, 0.42, 0.32, 0.08, 16, 22, 20, 24, 14, 6, 18, 5)
        }
        OrgTemplate::University => profile(0.3, 0.7, 0.22, 0.08, 6, 8, 24, 10, 3, 3, 6, 10),
        OrgTemplate::ManagedServiceProvider => {
            profile(0.5, 0.3, 0.28, 0.18, 22, 20, 14, 6, 4, 6, 10, 16)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn profile(
    maturity: f64,
    users: f64,
    endpoints: f64,
    servers: f64,
    admins: u32,
    services: u32,
    saas: u32,
    repos: u32,
    pipelines: u32,
    namespaces: u32,
    workloads: u32,
    segments: u32,
) -> TemplateProfile {
    TemplateProfile {
        maturity,
        user_share: users,
        endpoint_share: endpoints,
        server_share: servers,
        admin_per_thousand: f64::from(admins),
        service_per_thousand: f64::from(services),
        saas_per_thousand: f64::from(saas),
        repo_per_thousand: f64::from(repos),
        pipeline_per_thousand: f64::from(pipelines),
        namespace_per_thousand: f64::from(namespaces),
        workload_per_thousand: f64::from(workloads),
        segments,
    }
}
