use std::string::ToString;

use rocket;

use errors;
use config;
use controller::uuid as cuuid;
use response::admin::Admin;
use response::{MaybeResponse, MaybeResponse2, RegistryResponse};
use response::empty::Empty;
use response::layers::LayerExists;
use response::uuid::UuidResponse;
use response::uuidaccept::UuidAcceptResponse;
use response::catalog::Catalog;
use response::html::HTML;

use state;
use types::{DigestStruct, Layer};

pub fn routes() -> Vec<rocket::Route> {
    routes![
        get_v2root,
        get_homepage,
        get_manifest,
        check_image_manifest,
        get_blob,
        post_blob_uuid,
        check_existing_layer,
        get_upload_progress,
        put_blob,
        patch_blob,
        delete_upload,
        post_blob_upload,
        delete_blob,
        put_image_manifest,
        get_catalog,
        get_image_tags,
        delete_image_manifest,
        // admin routes
        admin_get_uuids,
    ]
}

pub fn errors() -> Vec<rocket::Catcher> {
    errors![err_400, err_404,]
}

#[error(400)]
fn err_400() -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}

#[error(404)]
fn err_404() -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}

/// Routes of a 2.0 Registry
///
/// Version Check of the registry
/// GET /v2/
///
/// # Responses
/// 200 - We Exist (and you are authenticated)
/// 401 - Please Authorize (WWW-Authenticate header with instuctions).
///
/// # Headers
/// Docker-Distribution-API-Version: registry/2.0
#[get("/v2")]
fn get_v2root() -> MaybeResponse<Empty> {
    MaybeResponse::build(Empty)
}

const ROOT_RESPONSE: &'static str = "<!DOCTYPE html><html><body>
<h1>Welcome to Lycaon, the King of Registries</h1>
</body></html>";

#[get("/")]
fn get_homepage<'a>() -> RegistryResponse<HTML<'a>> {
    RegistryResponse(HTML(ROOT_RESPONSE))
}

/*
---
Pulling an image
GET /v2/<name>/manifests/<reference>

# Parameters
name - The name of the image
reference - either a tag or a digest

# Client Headers
Accept: manifest-version

# Headers
Accept: manifest-version
?Docker-Content-Digest: digest of manifest file

# Returns
200 - return the manifest
404 - manifest not known to the registry
 */
#[get("/v2/<_name>/<_repo>/manifests/<reference>")]
fn get_manifest(_name: String, _repo: String, reference: String) -> MaybeResponse<Empty> {
    info!("Getting Manifest");
    match reference.as_str() {
        "good" => MaybeResponse::ok(Empty),
        _ => MaybeResponse::err(Empty),
    }
}
/*

---
Check for existence
HEAD /v2/<name>/manifests/<reference>

# Parameters
name - The name of the image
reference - either a tag or a digest

# Headers
Content-Length: size of manifest
?Docker-Content-Digest: digest of manifest file

# Returns
200 - manifest exists
404 - manifest does not exist
 */
#[head("/v2/<_name>/<_repo>/manifests/<_reference>")]
fn check_image_manifest(_name: String, _repo: String, _reference: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}

/*
---
Pulling a Layer
GET /v2/<name>/blobs/<digest>
name - name of the repository
digest - unique identifier for the blob to be downoaded

# Responses
200 - blob is downloaded
307 - redirect to another service for downloading[1]
 */
#[get("/v2/<_name>/<_repo>/blobs/<digest>")]
fn get_blob(_name: String, _repo: String, digest: String) -> MaybeResponse<Empty> {
    info!("Getting Blob");
    match digest.as_str() {
        "good" => MaybeResponse::ok(Empty),
        _ => MaybeResponse::err(Empty),
    }
}

/// Pushing a Layer
/// POST /v2/<name>/blobs/uploads/
/// name - name of repository
///
/// # Headers
/// Location: /v2/<name>/blobs/uploads/<uuid>
/// Range: bytes=0-<offset>
/// Content-Length: 0
/// Docker-Upload-UUID: <uuid>
///
/// # Returns
/// 202 - accepted
#[post("/v2/<_name>/<_repo>/blobs/uploads/<_uuid>")]
fn post_blob_uuid(_name: String, _repo: String, _uuid: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}

/*
---
Check for existing layer
HEAD /v2/<name>/blobs/<digest>
name - name of repository
digest - digest of blob to be checked

# Headers
Content-Length: <length of blob>
Docker-Content-Digest: <digest>

# Returns
200 - exists
404 - does not exist
 */

#[head("/v2/<name>/<repo>/blobs/<digest>")]
fn check_existing_layer(
    backend: rocket::State<config::BackendHandler>,
    name: String,
    repo: String,
    digest: String,
) -> MaybeResponse<LayerExists> {
    debug!("Handling LayerExists route");
    LayerExists::handle(backend, Layer { name, repo, digest })
        .map(|response| MaybeResponse::build(response))
        .map_err(|e| {
            warn!("{}", e);
            errors::Client::BLOB_UNKNOWN
        })
        .unwrap_or(MaybeResponse::build(LayerExists::False))
}

/*
---
Upload Progress
GET /v2/<name>/blobs/uploads/<uuid>
name - name of registry
uuid - unique id for the upload that is to be checked

# Client Headers
Host: <registry host>

# Headers
Location: /v2/<name>/blobs/uploads/<uuid>
Range: bytes=0-<offset>
Docker-Upload-UUID: <uuid>

# Returns
204
 */
#[get("/v2/<_name>/<_repo>/blobs/uploads/<_uuid>")]
fn get_upload_progress(_name: String, _repo: String, _uuid: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}
/*

---
Monolithic Upload
PUT /v2/<name>/blobs/uploads/<uuid>?digest=<digest>
Content-Length: <size of layer>
Content-Type: application/octet-stream

<Layer Binary Data>
---
Chunked Upload (Don't implement until Monolithic works)
Must be implemented as docker only supports this
PATCH /v2/<name>/blobs/uploads/<uuid>
Content-Length: <size of chunk>
Content-Range: <start of range>-<end of range>
Content-Type: application/octet-stream

<Layer Chunk Binary Data>
 */

#[put("/v2/<name>/<repo>/blobs/uploads/<uuid>?<digest>")] // capture digest query string
fn put_blob(
    config: rocket::State<config::BackendHandler>,
    name: String,
    repo: String,
    uuid: String,
    digest: DigestStruct,
) -> MaybeResponse<UuidAcceptResponse> {
    UuidAcceptResponse::handle(config, name, repo, uuid, digest)
        .map(|response| MaybeResponse::build(response))
        .or_else(|e| {
            warn!("Could not save image: {:?}", e);
            Err(e)
        })
        .unwrap_or(MaybeResponse::build(UuidAcceptResponse::UnknownError))
}

#[patch("/v2/<name>/<repo>/blobs/uploads/<uuid>", data = "<chunk>")]
fn patch_blob(
    handler: rocket::State<config::BackendHandler>,
    name: String,
    repo: String,
    uuid: String,
    chunk: rocket::data::Data,
) -> MaybeResponse<UuidResponse> {
    debug!("Checking if uuid is valid!");
    let exists = UuidResponse::uuid_exists(handler, &uuid);
    if let Ok(_) = exists {
        let absolute_file = state::uuid::scratch_path(&uuid);
        debug!("Streaming out to {}", absolute_file);
        let file = chunk.stream_to_file(absolute_file);

        match file {
            Ok(_) => {
                let right = match file.map(|x| x.to_string()) {
                    Ok(x) => x.parse::<u32>().unwrap(),
                    Err(_) => 0,
                };
                MaybeResponse::build(UuidResponse::Uuid {
                    uuid,
                    name,
                    repo,
                    left: 0,
                    right,
                })
            }
            Err(_) => MaybeResponse::build(UuidResponse::Empty),
        }
    } else {
        MaybeResponse::build(UuidResponse::Empty)
    }
}

/*


---
Cancelling an upload
DELETE /v2/<name>/blobs/uploads/<uuid>

 */

/// This route assumes that no more data will be uploaded to the specified uuid.
#[delete("/v2/<_name>/<_repo>/blobs/uploads/<uuid>")]
fn delete_upload(
    handler: rocket::State<config::BackendHandler>,
    _name: String,
    _repo: String,
    uuid: String,
) -> MaybeResponse2<UuidAcceptResponse> {
    MaybeResponse::build(UuidAcceptResponse::delete_upload(handler, &uuid))
}
/*
---
Cross repo blob mounting (validate how regularly this is used)
POST /v2/<name>/blobs/uploads/?mount=<digest>&from=<repository name>

 */

#[post("/v2/<name>/<repo>/blobs/uploads")]
fn post_blob_upload(
    handler: rocket::State<config::BackendHandler>,
    name: String,
    repo: String,
) -> MaybeResponse<UuidResponse> {
    UuidResponse::handle(handler, name, repo)
        .map_err(|e| {
            warn!("Uuid Generate: {}", e);
        })
        .map(|response| MaybeResponse::build(response))
        .unwrap_or(MaybeResponse::build(UuidResponse::Empty))
}
/*

---
Delete a layer
DELETE /v2/<name>/blobs/<digest>

 */
#[delete("/v2/<_name>/<_repo>/blobs/<_digest>")]
fn delete_blob(_name: String, _repo: String, _digest: String) -> MaybeResponse<Empty> {
    MaybeResponse::build(Empty)
}
/*

---
Pushing an image manifest
PUT /v2/<name>/manifests/<reference>
Content-Type: <manifest media type>

 */
#[put("/v2/<_name>/<_repo>/manifests/<_reference>")]
fn put_image_manifest(_name: String, _repo: String, _reference: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}
/*
---
Listing Repositories
GET /v2/_catalog

 */
#[get("/v2/_catalog")]
fn get_catalog() -> MaybeResponse<Catalog> {
    MaybeResponse::build(Catalog)
}
/*
---
Listing Image Tags
GET /v2/<name>/tags/list

 */
#[delete("/v2/<_name>/<_repo>/tags/list")]
fn get_image_tags(_name: String, _repo: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}
/*
---
Deleting an Image
DELETE /v2/<name>/manifests/<reference>

 */
#[delete("/v2/<_name>/<_repo>/manifests/<_reference>")]
fn delete_image_manifest(_name: String, _repo: String, _reference: String) -> MaybeResponse<Empty> {
    MaybeResponse::err(Empty)
}

#[get("/admin/uuids")]
fn admin_get_uuids(handler: rocket::State<config::BackendHandler>) -> MaybeResponse<Admin> {
    MaybeResponse::build(
        Admin::get_uuids(handler)
            .map(|uuids| {
                // oMaybeResponse::build(uuids)
                uuids
            })
            .unwrap_or(Admin::Uuids(vec![])),
    )
}

/*
---
[1]: Could possibly be used to redirect a client to a local cache
 */
