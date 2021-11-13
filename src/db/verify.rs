use anyhow::{bail, Result};
use bytes::Bytes;
use sequoia_openpgp::{
    parse::{stream::*, Parse},
    policy::StandardPolicy,
    Cert, KeyHandle,
};
use std::io::Read;
use std::path::Path;

pub fn verify_inrelease(cert_root: &Path, cert_filenames: &[String], msg: Bytes) -> Result<String> {
    let mut cert_paths = Vec::new();
    for cert_file in cert_filenames {
        let cert_path = cert_root.join(&cert_file);
        if cert_path.is_file() {
            cert_paths.push(cert_path);
        } else {
            bail!(
                "Public key file {} not found",
                console::style(cert_file).bold().to_string()
            );
        }
    }

    let verifier = InReleaseVerifier::new(&cert_paths)?;
    let p = &StandardPolicy::new();
    let mut v = VerifierBuilder::from_bytes(&msg)?.with_policy(p, None, verifier)?;
    let mut content = String::new();
    v.read_to_string(&mut content)?;

    Ok(content)
}

pub struct InReleaseVerifier {
    certs: Vec<Cert>,
}

impl InReleaseVerifier {
    pub fn new<P: AsRef<Path>>(cert_paths: &[P]) -> Result<Self> {
        let mut certs: Vec<Cert> = Vec::new();
        for path in cert_paths.iter() {
            certs.push(Cert::from_file(path)?);
        }
        Ok(InReleaseVerifier { certs })
    }
}

impl VerificationHelper for InReleaseVerifier {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
        let mut certs = Vec::new();
        for id in ids {
            for cert in self.certs.iter() {
                if &cert.key_handle() == id {
                    certs.push(cert.clone());
                }
            }
        }
        Ok(certs)
    }

    fn check(&mut self, structure: MessageStructure) -> Result<()> {
        for layer in structure.into_iter() {
            if let MessageLayer::SignatureGroup { results } = layer {
                for r in results {
                    if let Err(e) = r {
                        bail!("InRelease has bad signature: {}", e);
                    }
                }
            } else {
                bail!("Malformed PGP signature, InRelease should only be signed")
            }
        }

        Ok(())
    }
}
