create function api.recent(n integer) returns setof api.notes language sql stable as 'select * from api.notes order by id desc limit n';
grant execute on function api.note_count() to reader;
grant execute on function api.recent(integer) to reader;
grant select on table api.notes to reader;
grant usage on schema api to reader;
revoke execute on function api.purge() from public;
revoke execute on function api.recent(integer) from public;
