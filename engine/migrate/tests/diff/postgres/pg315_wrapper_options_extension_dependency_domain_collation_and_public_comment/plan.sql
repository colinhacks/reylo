alter domain code rename to code__old;
create domain code as text collate "C";
drop domain code__old;
alter foreign table ft alter column a options (add column_name 'b');
comment on schema public is 'the project''s';
alter function f() depends on extension citext;
