comment on function api.touch() is null;
comment on constraint handle_short on api.users is 'Handles fit in a URL';
comment on index api.users_status is 'For the status filter';
comment on policy users_all on api.users is 'Everyone sees everything';
comment on schema api is 'What the API serves';
comment on trigger users_touch on api.users is 'Runs on every update';
comment on domain api.handle is 'A lowercase handle';
comment on type api.status is 'Whether a user is active';
