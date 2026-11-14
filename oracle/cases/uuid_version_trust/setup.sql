-- A user's `uuid_extract_version` over a domain answers where the CHECK calls it, and holds the
-- column to nothing: the catalog lists it among the CHECK's calls, where a built-in is pinned.
create domain myuuid as uuid;
create function public.uuid_extract_version(myuuid) returns smallint language sql immutable as 'select 7::smallint';
create table mine (id myuuid check (uuid_extract_version(id) = 7 and uuid_extract_version(id) is not null));
-- `(x).*` of a type with no column expands to none, so the targets and the columns are out of line.
create type empty as ();
create table w (a uuid, b uuid);
-- A user's `now` of one argument that returns a row whose column is named `uuidv7`.
create type comp as (uuidv7 uuid);
create function public.now(x int) returns comp language sql as $$ select row('00000000-0000-0000-0000-000000000000')::comp $$;
