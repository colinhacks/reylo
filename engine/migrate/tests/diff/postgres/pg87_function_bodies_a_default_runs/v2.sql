create schema app;
create function app.api_request_headers() returns jsonb language sql stable as $f$ select coalesce(current_setting('request.headers', true), '{}')::jsonb $f$;
create function app.api_request_header(header_name text) returns text language sql stable as $f$ select app.api_request_headers() ->> header_name $f$;
create function app.api_request_client_info() returns text language sql stable as $f$ select coalesce(app.api_request_header('user-agent'), 'unknown') $f$;
create table app.downloads (id bigint primary key, client_info text default app.api_request_client_info());
create function app.normalize_title_inner(input text) returns text language sql immutable as $f$ select lower(btrim(input)) $f$;
create function app.normalize_title(input text) returns text language sql immutable as $f$ select app.normalize_title_inner(input) $f$;
create table app.documents (id bigint primary key, title text not null);
create index documents_title_norm_idx on app.documents ((app.normalize_title(title)));
