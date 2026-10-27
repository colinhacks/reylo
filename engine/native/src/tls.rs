//! TLS as libpq's `sslmode` and `sslrootcert` ask for it, over rustls: `require` encrypts and checks
//! nothing unless a root certificate file is named; `verify-ca` checks the certificate chain;
//! `verify-full` and `sslrootcert=system` check the chain and the host name. Without a root
//! certificate file, the chain is checked against the operating system's certificates, or in the
//! WebAssembly component against the Mozilla roots webpki-roots carries and those
//! `NODE_EXTRA_CA_CERTS` names, as Node's own TLS checks it. A server certificate that
//! is itself one of the root file's certificates, as a self-signed server's is when its own
//! certificate is named as the root, is accepted as libpq accepts it. A client certificate
//! (`sslcert` and `sslkey`) is presented where the URL names one.

// The MySQL program reads only `failed`: mysql_async makes its own TLS configuration.
#![cfg_attr(not(feature = "postgres"), allow(dead_code))]

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use std::sync::Arc;

/// How much of the server's certificate is checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    /// Nothing: the connection is encrypted, as libpq's `require` without a root certificate file.
    Nothing,
    /// The chain, not the host name: `verify-ca`.
    Chain,
    /// The chain and the host name: `verify-full`.
    Full,
}

/// A client certificate: its PEM file and its key's, as libpq's `sslcert` and `sslkey` name them.
pub struct Identity<'a> {
    pub cert: &'a str,
    pub key: &'a str,
    /// libpq's `sslpassword`, which only an encrypted key needs.
    pub password: bool,
}

/// A client configuration that checks what `check` says, against the certificates of `root` (a PEM
/// file) or, with none, the operating system's, and presents `identity` where there is one.
pub fn config(check: Check, root: Option<&str>, identity: Option<Identity<'_>>) -> Result<ClientConfig, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier: Arc<dyn ServerCertVerifier> = match (check, root) {
        (Check::Nothing, _) => Arc::new(Unchecked(provider.clone())),
        (_, Some(file)) => {
            let certs = CertificateDer::pem_file_iter(file)
                .map_err(|e| format!("cannot read sslrootcert {file}: {e}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("cannot read sslrootcert {file}: {e}"))?;
            let mut roots = RootCertStore::empty();
            for cert in &certs {
                roots.add(cert.clone()).map_err(|e| format!("sslrootcert {file}: {e}"))?;
            }
            let chain = rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone()).build().map_err(|e| e.to_string())?;
            Arc::new(OrRoot { chain, roots: certs, name: check == Check::Full })
        }
        #[cfg(not(target_family = "wasm"))]
        (_, None) => Arc::new(rustls_platform_verifier::Verifier::new(provider.clone()).map_err(|e| e.to_string())?),
        // A WebAssembly engine has no operating system's store to read: it checks against the
        // Mozilla roots webpki-roots carries, as Node's own TLS does, with those Node adds from
        // `NODE_EXTRA_CA_CERTS` (a company's root, say), and a root file names any other.
        #[cfg(target_family = "wasm")]
        (_, None) => {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            if let Some(certs) = std::env::var("NODE_EXTRA_CA_CERTS").ok().and_then(|file| CertificateDer::pem_file_iter(file).ok()) {
                for cert in certs.flatten() {
                    let _ = roots.add(cert);
                }
            }
            rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone()).build().map_err(|e| e.to_string())?
        }
    };
    let verifier = if check == Check::Chain { Arc::new(AnyName(verifier)) } else { verifier };
    let builder = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().map_err(|e| e.to_string())?.dangerous().with_custom_certificate_verifier(verifier);
    let Some(identity) = identity else {
        return Ok(builder.with_no_client_auth());
    };
    let certs = CertificateDer::pem_file_iter(identity.cert)
        .and_then(|certs| certs.collect::<Result<Vec<_>, _>>())
        .map_err(|e| format!("cannot read sslcert {}: {e}", identity.cert))?;
    let key = PrivateKeyDer::from_pem_file(identity.key).map_err(|e| {
        if identity.password {
            format!("cannot read sslkey {}: {e}; a key encrypted with sslpassword is not read, so name the key decrypted", identity.key)
        } else {
            format!("cannot read sslkey {}: {e}", identity.key)
        }
    })?;
    builder.with_client_auth_cert(certs, key).map_err(|e| format!("sslcert {} and sslkey {}: {e}", identity.cert, identity.key))
}

/// Whether `e`, or what caused it, is the TLS handshake's failure: a certificate the check refused,
/// or a server that answered TLS with something else. That is a setting to fix, not a database that
/// cannot be reached. An I/O error hides the error it wraps from its `source`, so that is read too.
pub fn failed(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(e);
    while let Some(err) = next {
        if err.is::<Error>() || err.downcast_ref::<std::io::Error>().and_then(std::io::Error::get_ref).is_some_and(|inner| inner.is::<Error>()) {
            return true;
        }
        next = err.source();
    }
    false
}

/// Accepts any certificate: the connection is encrypted and the server unchecked.
#[derive(Debug)]
struct Unchecked(Arc<CryptoProvider>);

impl ServerCertVerifier for Unchecked {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// A server certificate that is byte for byte one of the roots, accepted (with its host name checked
/// where `name` says so), as libpq accepts a self-signed server certificate named as its own root,
/// which webpki refuses as a CA used as an end entity; any other is checked as `chain` checks it.
/// Like libpq, the roots are trusted as given: such a certificate's dates are not read.
#[derive(Debug)]
struct OrRoot {
    chain: Arc<dyn ServerCertVerifier>,
    roots: Vec<CertificateDer<'static>>,
    name: bool,
}

impl ServerCertVerifier for OrRoot {
    fn verify_server_cert(&self, end: &CertificateDer<'_>, intermediates: &[CertificateDer<'_>], name: &ServerName<'_>, ocsp: &[u8], now: UnixTime) -> Result<ServerCertVerified, Error> {
        if !self.roots.iter().any(|root| root.as_ref() == end.as_ref()) {
            return self.chain.verify_server_cert(end, intermediates, name, ocsp, now);
        }
        if self.name {
            rustls::client::verify_server_name(&rustls::server::ParsedCertificate::try_from(end)?, name)?;
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.chain.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.chain.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.chain.supported_verify_schemes()
    }
}

/// Checks the chain as the inner verifier does, whatever host name the certificate is for.
#[derive(Debug)]
struct AnyName(Arc<dyn ServerCertVerifier>);

impl ServerCertVerifier for AnyName {
    fn verify_server_cert(&self, end: &CertificateDer<'_>, intermediates: &[CertificateDer<'_>], name: &ServerName<'_>, ocsp: &[u8], now: UnixTime) -> Result<ServerCertVerified, Error> {
        match self.0.verify_server_cert(end, intermediates, name, ocsp, now) {
            Err(Error::InvalidCertificate(CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. })) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}
