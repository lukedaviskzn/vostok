use std::{io::{self, Read, Write}, net::TcpStream, sync::Arc};

use rustls::{client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier}, pki_types::{DnsName, IpAddr, ServerName}};

pub const SCHEME: &str = "gemini";
pub const PORT: u16 = 1965;

/// Trust on first use certificate verifier. Currently just accepts all.
#[derive(Debug)]
pub struct Tofu; // todo: verify certs

impl ServerCertVerifier for Tofu {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA1,
            rustls::SignatureScheme::ECDSA_SHA1_Legacy,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}

pub fn request(url: &url::Url) -> io::Result<Response> {
    log::debug!("requesting {url}");

    if url.scheme() == SCHEME {
        if url.has_host() {
            let host_str = url.host_str().expect("unreachable");
            let host = url.host().expect("unreachable");

            let mut socket = TcpStream::connect((host_str, url.port_or_known_default().unwrap_or(PORT)))?;

            let config = rustls::ClientConfig::builder().dangerous()
                .with_custom_certificate_verifier(Arc::new(Tofu))
                .with_no_client_auth();

            let name = match host {
                url::Host::Domain(domain) => ServerName::DnsName(DnsName::try_from(domain.to_owned()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?),
                url::Host::Ipv4(ip) => ServerName::IpAddress(IpAddr::V4(ip.into())),
                url::Host::Ipv6(ip) => ServerName::IpAddress(IpAddr::V6(ip.into())),
            };
            
            let mut client = rustls::ClientConnection::new(Arc::new(config), name).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
            let mut stream = rustls::Stream::new(&mut client, &mut socket);

            stream.write_all(format!("{url}\r\n").as_bytes())?;
            
            let mut response = Vec::new();

            stream.read_to_end(&mut response)?;

            let response = String::from_utf8(response).map_err(|_| io::ErrorKind::InvalidData)?;
            let response = RawResponse::try_from(response)?;
            
            return Response::try_from(response);
        }
    }

    Err(io::ErrorKind::Unsupported.into())
}

// 20 text/gemini\r\n# Project Gemini\n\n## Gemini in 100 words\n\nGemini is a new internet technology supporting an electronic library of interconnected text documents.  That's not a new idea, but it's not old fashioned either.  It's timeless, and deserves tools which treat it as a first class concept, not a vestigial corner case.  Gemini isn't about innovation or disruption, it's about providing some respite for those who feel the internet has been disrupted enough already.  We're not out to change the world or destroy other technologies.  We are out to build a lightweight online space where documents are just documents, in the interests of every reader's privacy, attention and bandwidth.\n\n=> docs/faq.gmi\tIf you'd like to know more, read our FAQ\n=> https://www.youtube.com/watch?v=DoEI6VzybDk\tOr, if you'd prefer, here's a video overview\n\n## Official resources\n\n=> news/\tProject Gemini news\n=> docs/\tProject Gemini documentation\n=> history/\tProject Gemini history\n=> software/\tKnown Gemini software\n\nAll content at geminiprotocol.net is CC BY-NC-ND 4.0 licensed unless stated otherwise:\n=> https://creativecommons.org/licenses/by-nc-nd/4.0/\tCC Attribution-NonCommercial-NoDerivs 4.0 International\n
struct RawResponse {
    status: u8,
    meta: String,
    body: String,
}

impl TryFrom<String> for RawResponse {
    type Error = io::Error;

    fn try_from(mut value: String) -> io::Result<RawResponse> {
        let mut meta = value.split_off(2);
        let status = value.parse().map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;

        if meta.starts_with(' ') {
            meta = meta.split_off(1);
        }
        
        let crlf = meta.find("\r\n").ok_or(io::Error::from(io::ErrorKind::InvalidData))?;
        let body = meta.split_off(crlf+2);
        // remove CRLF
        meta.pop(); meta.pop();
        
        Ok(RawResponse {
            status,
            meta,
            body,
        })
    }
}

#[derive(Debug)]
pub struct Response {
    status: u8,
    content: ResponseContent,
}

impl Response {
    pub fn status(&self) -> u8 {
        self.status
    }

    pub fn content(&self) -> &ResponseContent {
        &self.content
    }
}

impl TryFrom<RawResponse> for Response {
    type Error = io::Error;

    fn try_from(value: RawResponse) -> io::Result<Response> {
        Ok(Response {
            status: value.status,
            content: match value.status / 10 {
                1 => ResponseContent::InputExpected { prompt: value.meta },
                2 => ResponseContent::Success { mimetype: value.meta, body: value.body },
                3 => ResponseContent::Redirection { uri: value.meta },
                4 => ResponseContent::TemporaryFailure { error: value.meta },
                5 => ResponseContent::PermanentFailure { error: value.meta },
                6 => ResponseContent::ClientCertifiates { error: value.meta },
                _ => return Err(io::ErrorKind::InvalidData.into()),
            },
        })
    }
}

#[derive(Debug)]
pub enum ResponseContent {
    InputExpected {
        prompt: String,
    },
    Success {
        mimetype: String,
        body: String,
    },
    Redirection {
        uri: String,
    },
    TemporaryFailure {
        error: String,
    },
    PermanentFailure {
        error: String,
    },
    ClientCertifiates {
        error: String,
    },
}
