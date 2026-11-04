use super::*;

pub(super) async fn dispatch(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    use crate::commands::client_software as software;
    let result = match request.command.as_str() {
        "read_client_software" => as_json(software::read_client_software(app.clone()).await),
        "select_client_software" => as_json(software::select_client_software(app.clone(), argument(&request.args, "client")?).await),
        "refresh_client_software" => as_json(software::refresh_client_software(app.clone(), argument(&request.args, "force")?).await),
        "prepare_client_update" => as_json(software::prepare_client_update(app.clone(), argument(&request.args, "installationId")?, argument(&request.args, "revision")?).await),
        "start_client_update" => as_json(software::start_client_update(app.clone(), argument(&request.args, "planId")?).await),
        "read_client_update" => as_json(software::read_client_update(app.clone()).await),
        "cancel_client_update" => as_json(software::cancel_client_update(app.clone(), argument(&request.args, "id")?).await),
        _ => return Ok(None),
    };
    result.map(Some)
}
