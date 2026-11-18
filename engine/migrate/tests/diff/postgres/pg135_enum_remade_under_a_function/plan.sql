alter type mood rename to mood__old;
create type mood as enum ('happy', 'ok', 'sad');
create or replace function score(m mood)
 returns integer
 language sql
 immutable
as $function$select 1$function$;
alter table t alter column m type mood using m::text::mood;
drop function score(mood__old);
drop type mood__old;
