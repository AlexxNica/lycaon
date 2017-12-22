use failure::Error;
use rocket::State;
use rocket::http::{Header, Status};
use rocket::response::{Responder, Response};
use rocket::request::Request;

use config;
use errors;
use controller::uuid as cuuid;
use grpc::backend;
use types;

const BASE_URL: &str = "http://localhost:8000";

#[derive(Debug, Serialize)]
pub enum UuidAcceptResponse {
    DigestMismatch,
    UuidAccept {
        uuid: String,
        digest: String,
        name: String,
        repo: String,
    },
    UuidDelete,
    UnknownError,
}

impl UuidAcceptResponse {
    /// Grab the temporary upload and transfer it into a permanent layer
    pub fn handle(
        handler: State<config::BackendHandler>,
        name: String,
        repo: String,
        uuid: String,
        digest: types::DigestStruct,
    ) -> Result<UuidAcceptResponse, Error> {
        let backend = handler.backend();
        let mut req = backend::CommitLayer::new();
        req.set_uuid(uuid);
        req.set_digest(digest.digest);
        let response = backend.commit_upload(req)?;
        match response.get_success() {
            true => Ok(UuidAcceptResponse::UuidDelete),
            false => Err(errors::Client::DIGEST_INVALID.into()),
        }
    }

    pub fn delete_upload(
        handler: State<config::BackendHandler>,
        uuid: &str,
    ) -> Result<UuidAcceptResponse, Error> {
        let backend = handler.backend();
        let mut req = backend::Layer::new();
        req.set_digest(uuid.to_owned());

        let response = backend.cancel_upload(req)?;

        match response.get_success() {
            true => Ok(UuidAcceptResponse::UuidDelete),
            false => Err(errors::Client::BLOB_UPLOAD_UNKNOWN.into()),
        }
    }
}

impl<'r> Responder<'r> for UuidAcceptResponse {
    fn respond_to(self, _req: &Request) -> Result<Response<'r>, Status> {
        use self::UuidAcceptResponse::*;

        match self {
            UuidAccept {
                name,
                repo,
                digest,
                uuid: _,
            } => {
                let location = format!("{}/v2/{}/{}/blobs/{}", BASE_URL, name, repo, digest);
                let location = Header::new("Location", location);
                let digest = Header::new("Docker-Content-Digest", digest);
                Response::build()
                    .status(Status::Created)
                    .header(location)
                    .header(digest)
                    .ok()
            }
            DigestMismatch => {
                debug!("Digest mismatched");
                Response::build().status(Status::NotFound).ok()
            }
            UuidDelete => Response::build().status(Status::NoContent).ok(),
            UnknownError => Response::build().status(Status::NotFound).ok(),
        }
    }
}

#[cfg(test)]
mod test {
    use rocket::http::Status;
    use response::uuid::UuidResponse;

    use test::test_helpers::test_route;
    fn build_response() -> UuidResponse {
        UuidResponse::Uuid {
            // TODO: keep this as a real Uuid!
            uuid: String::from("whatever"),
            name: String::from("moredhel"),
            repo: String::from("test"),
            left: 0,
            right: 0,
        }
    }

    #[test]
    fn uuid_uuid() {
        let response = test_route(build_response());
        let headers = response.headers();
        assert_eq!(response.status(), Status::Accepted);
        assert!(headers.contains("Docker-Upload-UUID"));
        assert!(headers.contains("Location"));
        assert!(headers.contains("Range"));
    }

    #[test]
    fn uuid_empty() {
        let response = test_route(UuidResponse::Empty);
        assert_eq!(response.status(), Status::NotFound);
    }
}
