create extension pgcrypto;
create extension postgres_fdw;
revoke execute on function digest(text, text) from public;
grant execute on function digest(text, text) to diff_reader;
grant usage on foreign data wrapper postgres_fdw to diff_reader;
revoke usage on language plpgsql from public;
comment on language plpgsql is 'procedures';
comment on extension pgcrypto is 'hashes';
