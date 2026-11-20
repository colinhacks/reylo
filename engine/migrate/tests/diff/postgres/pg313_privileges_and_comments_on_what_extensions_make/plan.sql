grant execute on routine digest(pg_catalog.text,pg_catalog.text) to diff_reader;
grant usage on foreign data wrapper postgres_fdw to diff_reader;
revoke execute on routine digest(pg_catalog.text,pg_catalog.text) from public;
revoke usage on language plpgsql from public;
comment on extension pgcrypto is 'hashes';
comment on language plpgsql is 'procedures';
