//! Microsoft Entra ID sign-in for SQL Server. The URL's `authentication=` names how the program
//! gets an access token for Azure SQL, as Microsoft's connection strings do, and tiberius sends the
//! token in the login. The token comes from Microsoft's identity crate, `azure_identity`, as each of
//! Microsoft's own drivers takes its tokens from MSAL or azure-identity rather than getting them
//! itself.
//!
//! - `ActiveDirectoryDefault`: a service principal the environment names (`AZURE_TENANT_ID`,
//!   `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`), a workload identity
//!   (`AZURE_FEDERATED_TOKEN_FILE`), the Azure CLI, the Azure Developer CLI, then the host's
//!   managed identity: the first that gives a token.
//! - `ActiveDirectoryAzureCli`: the Azure CLI's sign-in, `az login`.
//! - `ActiveDirectoryManagedIdentity` (or `ActiveDirectoryMSI`): the Azure host's identity; the
//!   URL's user, where it has one, is the client id of a user-assigned identity.
//! - `ActiveDirectoryWorkloadIdentity`: the token file Kubernetes mounts, with `AZURE_CLIENT_ID`
//!   and `AZURE_TENANT_ID`.
//! - `ActiveDirectoryServicePrincipal`: the URL's user is the application's client id and its
//!   password the client secret, in the tenant `tenantId=` or `AZURE_TENANT_ID` names.
//!
//! A credential is made once a process for each method, so the connections a command opens share
//! its tokens.

use azure_core::credentials::{Secret, TokenCredential};
use azure_identity::{AzureCliCredential, AzureDeveloperCliCredential, ClientSecretCredential, ManagedIdentityCredential, ManagedIdentityCredentialOptions, UserAssignedId, WorkloadIdentityCredential};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// The scope of a token for Azure SQL.
const SCOPE: &str = "https://database.windows.net/.default";

/// How long the host's managed identity may take to answer, where `ActiveDirectoryDefault` tries
/// it last: off Azure, nothing answers its endpoint.
const MANAGED_IDENTITY_TIMEOUT: Duration = Duration::from_secs(5);

/// How the program gets an Entra ID token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entra {
    Default,
    AzureCli,
    /// A user-assigned identity's client id, or none for the system-assigned identity.
    ManagedIdentity(Option<String>),
    WorkloadIdentity,
    ServicePrincipal { tenant: String, client_id: String, secret: String },
}

impl Entra {
    /// The sign-in `authentication=` names, from the URL's user, password and `tenantId=`: none for
    /// a SQL Server login, as `SqlPassword` or no `authentication=` at all.
    pub fn named(method: Option<&str>, user: &str, password: &str, tenant: Option<String>) -> Result<Option<Entra>, String> {
        let Some(method) = method else { return Ok(None) };
        Ok(Some(match method.to_ascii_lowercase().replace([' ', '-'], "").as_str() {
            "sqlpassword" => return Ok(None),
            "activedirectorydefault" => Entra::Default,
            "activedirectoryazurecli" => Entra::AzureCli,
            "activedirectorymanagedidentity" | "activedirectorymsi" => Entra::ManagedIdentity(Some(user.to_string()).filter(|u| !u.is_empty())),
            "activedirectoryworkloadidentity" => Entra::WorkloadIdentity,
            "activedirectoryserviceprincipal" => {
                let tenant = tenant.or_else(|| std::env::var("AZURE_TENANT_ID").ok()).filter(|t| !t.is_empty());
                match (user.is_empty(), password.is_empty(), tenant) {
                    (false, false, Some(tenant)) => Entra::ServicePrincipal { tenant, client_id: user.to_string(), secret: password.to_string() },
                    (_, _, None) => return Err("authentication=ActiveDirectoryServicePrincipal names its tenant with tenantId= in the URL, or AZURE_TENANT_ID".into()),
                    _ => return Err("authentication=ActiveDirectoryServicePrincipal takes the application's client id as the URL's user and its client secret as the password".into()),
                }
            }
            "activedirectorypassword" | "activedirectoryinteractive" | "activedirectorydevicecodeflow" | "activedirectoryintegrated" => {
                return Err(format!(
                    "authentication={method} is not one Reylo signs in with: use ActiveDirectoryDefault, ActiveDirectoryAzureCli (after az login), ActiveDirectoryManagedIdentity, ActiveDirectoryWorkloadIdentity or ActiveDirectoryServicePrincipal"
                ))
            }
            _ => return Err(format!("authentication={method} names no sign-in: write SqlPassword, ActiveDirectoryDefault, ActiveDirectoryAzureCli, ActiveDirectoryManagedIdentity, ActiveDirectoryWorkloadIdentity or ActiveDirectoryServicePrincipal")),
        }))
    }

    /// The method's name as the URL writes it, for a message.
    pub fn name(&self) -> &'static str {
        match self {
            Entra::Default => "ActiveDirectoryDefault",
            Entra::AzureCli => "ActiveDirectoryAzureCli",
            Entra::ManagedIdentity(_) => "ActiveDirectoryManagedIdentity",
            Entra::WorkloadIdentity => "ActiveDirectoryWorkloadIdentity",
            Entra::ServicePrincipal { .. } => "ActiveDirectoryServicePrincipal",
        }
    }

    /// A key for the process's credentials, which holds no secret.
    fn key(&self) -> String {
        match self {
            Entra::ManagedIdentity(id) => format!("mi:{}", id.as_deref().unwrap_or_default()),
            Entra::ServicePrincipal { tenant, client_id, .. } => format!("sp:{tenant}:{client_id}"),
            other => other.name().to_string(),
        }
    }
}

/// The process's credentials, by `Entra::key`.
/// One credential of a method's chain, and how long its answer may take.
#[derive(Clone)]
struct Link {
    credential: Arc<dyn TokenCredential>,
    wait: Option<Duration>,
}

/// The credentials a method tries, in order, and why each of the default chain's that could not be
/// made was passed over.
#[derive(Clone)]
struct Chain {
    links: Vec<Link>,
    passed_over: Vec<String>,
}

fn credentials() -> &'static Mutex<HashMap<String, Chain>> {
    static CREDENTIALS: OnceLock<Mutex<HashMap<String, Chain>>> = OnceLock::new();
    CREDENTIALS.get_or_init(Default::default)
}

fn link(credential: Arc<dyn TokenCredential>) -> Link {
    Link { credential, wait: None }
}

/// The credentials a method tries, in order.
fn made(method: &Entra) -> azure_core::Result<Chain> {
    let managed = |id: Option<&String>| {
        let mut options = ManagedIdentityCredentialOptions::default();
        options.user_assigned_id = id.cloned().map(UserAssignedId::ClientId);
        ManagedIdentityCredential::new(Some(options)).map(|c| c as Arc<dyn TokenCredential>)
    };
    let one = |c: Arc<dyn TokenCredential>| Chain { links: vec![link(c)], passed_over: Vec::new() };
    Ok(match method {
        Entra::AzureCli => one(AzureCliCredential::new(None)?),
        Entra::ManagedIdentity(id) => one(managed(id.as_ref())?),
        Entra::WorkloadIdentity => one(WorkloadIdentityCredential::new(None)?),
        Entra::ServicePrincipal { tenant, client_id, secret } => one(ClientSecretCredential::new(tenant, client_id.clone(), Secret::new(secret.clone()), None)?),
        Entra::Default => {
            let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
            let mut chain = Chain { links: Vec::new(), passed_over: Vec::new() };
            // A credential that cannot be made here (a service principal or a workload identity the
            // environment names only in part, a managed identity of a kind the crate does not take,
            // as Cloud Shell's) is passed over, as the next in the chain may still sign in.
            let mut add = |name: &str, made: azure_core::Result<Arc<dyn TokenCredential>>, wait: Option<Duration>| match made {
                Ok(credential) => chain.links.push(Link { credential, wait }),
                Err(e) => chain.passed_over.push(format!("{name}: {e}")),
            };
            if let (Some(tenant), Some(client_id), Some(secret)) = (env("AZURE_TENANT_ID"), env("AZURE_CLIENT_ID"), env("AZURE_CLIENT_SECRET")) {
                add("the environment's service principal", ClientSecretCredential::new(&tenant, client_id, Secret::new(secret), None).map(|c| c as Arc<dyn TokenCredential>), None);
            }
            if env("AZURE_FEDERATED_TOKEN_FILE").is_some() {
                add("the workload identity", WorkloadIdentityCredential::new(None).map(|c| c as Arc<dyn TokenCredential>), None);
            }
            add("the Azure CLI", AzureCliCredential::new(None).map(|c| c as Arc<dyn TokenCredential>), None);
            add("the Azure Developer CLI", AzureDeveloperCliCredential::new(None).map(|c| c as Arc<dyn TokenCredential>), None);
            // Off Azure, nothing answers the managed identity's endpoint.
            add("the host's managed identity", managed(None), Some(MANAGED_IDENTITY_TIMEOUT));
            chain
        }
    })
}

/// An access token for Azure SQL, by `method`: the first of its credentials that gives one.
pub async fn token(method: &Entra) -> Result<String, String> {
    let key = method.key();
    let chain = {
        let mut all = credentials().lock().unwrap_or_else(|e| e.into_inner());
        match all.get(&key) {
            Some(chain) => chain.clone(),
            None => {
                let chain = made(method).map_err(|e| format!("Entra ID sign-in ({}) could not start: {e}", method.name()))?;
                all.insert(key, chain.clone());
                chain
            }
        }
    };
    let mut failures = chain.passed_over.clone();
    for Link { credential, wait } in &chain.links {
        let attempt = credential.get_token(&[SCOPE], None);
        let answer = match wait {
            Some(wait) => match tokio::time::timeout(*wait, attempt).await {
                Ok(answer) => answer,
                Err(_) => {
                    failures.push(format!("the host's managed identity gave no answer within {} seconds", wait.as_secs()));
                    continue;
                }
            },
            None => attempt.await,
        };
        match answer {
            Ok(token) => return Ok(token.token.secret().to_string()),
            Err(e) => failures.push(e.to_string()),
        }
    }
    Err(format!("Entra ID sign-in ({}) got no token: {}", method.name(), failures.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_method() {
        assert_eq!(Entra::named(None, "sa", "x", None), Ok(None));
        assert_eq!(Entra::named(Some("SqlPassword"), "sa", "x", None), Ok(None));
        assert_eq!(Entra::named(Some("ActiveDirectoryDefault"), "", "", None), Ok(Some(Entra::Default)));
        assert_eq!(Entra::named(Some("activedirectoryazurecli"), "", "", None), Ok(Some(Entra::AzureCli)));
        assert_eq!(Entra::named(Some("Active Directory Managed Identity"), "", "", None), Ok(Some(Entra::ManagedIdentity(None))));
        assert_eq!(Entra::named(Some("ActiveDirectoryMSI"), "8c2e", "", None), Ok(Some(Entra::ManagedIdentity(Some("8c2e".into())))));
        assert_eq!(
            Entra::named(Some("ActiveDirectoryServicePrincipal"), "app", "secret", Some("tenant".into())),
            Ok(Some(Entra::ServicePrincipal { tenant: "tenant".into(), client_id: "app".into(), secret: "secret".into() }))
        );
        assert!(Entra::named(Some("ActiveDirectoryServicePrincipal"), "app", "", Some("tenant".into())).unwrap_err().contains("client secret as the password"));
        assert!(Entra::named(Some("ActiveDirectoryPassword"), "a", "b", None).unwrap_err().starts_with("authentication=ActiveDirectoryPassword is not one Reylo signs in with"));
        assert!(Entra::named(Some("Kerberos"), "", "", None).unwrap_err().starts_with("authentication=Kerberos names no sign-in"));
    }

    /// The Azure CLI's token, through a stand-in `az` that answers as `az account get-access-token`
    /// does and says which scope it was asked for.
    #[cfg(unix)]
    #[tokio::test]
    async fn takes_the_azure_cli_token() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("reylo-entra-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let az = dir.join("az");
        std::fs::write(&az, "#!/bin/sh\necho \"$*\" > \"$(dirname \"$0\")/asked\"\necho '{\"accessToken\": \"a-token\", \"expires_on\": 4102444800, \"tokenType\": \"Bearer\"}'\n").unwrap();
        std::fs::set_permissions(&az, std::fs::Permissions::from_mode(0o755)).unwrap();
        // No other test of this crate reads PATH or the identity's variables, or runs a program.
        std::env::set_var("PATH", format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default()));
        assert_eq!(token(&Entra::AzureCli).await, Ok("a-token".to_string()));
        assert_eq!(std::fs::read_to_string(dir.join("asked")).unwrap().trim(), "account get-access-token -o json --scope https://database.windows.net/.default");
        // A managed identity the crate cannot make, as Cloud Shell's, leaves the rest of the
        // default chain to sign in.
        std::env::set_var("MSI_ENDPOINT", "http://localhost:50342/oauth2/token");
        std::env::remove_var("MSI_SECRET");
        assert_eq!(token(&Entra::Default).await, Ok("a-token".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
