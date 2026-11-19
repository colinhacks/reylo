-- file 1 of 2
set check_function_bodies = off;
create function app.api_request_client_info() returns text language sql stable as $f$ select coalesce(app.api_request_header('user-agent'), 'unknown') $f$;
create function app.api_request_header(header_name text) returns text language sql stable as $f$ select app.api_request_headers() ->> header_name $f$;
create function app.api_request_headers() returns jsonb language sql stable as $f$ select coalesce(current_setting('request.headers', true), '{}')::jsonb $f$;
create function app.normalize_title(input text) returns text language sql immutable as $f$ select app.normalize_title_inner(input) $f$;
create function app.normalize_title_inner(input text) returns text language sql immutable as $f$ select lower(btrim(input)) $f$;
alter table app.downloads add column client_info text default app.api_request_client_info();
-- file 2 of 2
create index concurrently documents_title_norm_idx on app.documents ((app.normalize_title(title)));
