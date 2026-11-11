-- A write's rows, where what bounds them is more than a key in its WHERE. A `from` or a `using` is a
-- join, but a target row is written once whatever it joins, so a key of the target pinned to a value,
-- or to a column of an item of one row, bounds what RETURNING returns. A BEFORE row trigger skips a
-- row only where it returns NULL, which its PL/pgSQL body says.
create table accounts (id int primary key, name text not null);
create table users (id int primary key, account_id int not null references accounts (id), email text unique, name text);
create table notes (id int primary key, user_id int not null, body text not null);
insert into accounts values (1, 'a'), (2, 'b');
insert into users values (1, 1, 'a@x', 'n'), (2, 2, null, null), (3, 1, 'c@x', 'c');
insert into notes values (1, 1, 'x'), (2, 1, 'y'), (3, 2, 'z');

create function stamp() returns trigger language plpgsql as $$ begin new.at := now(); return new; end $$;
create function stamp_unless_set() returns trigger language plpgsql as $$
begin
  if current_setting('app.keep', true) = 'on' then
    return new;
  end if;
  if new.at is not distinct from old.at then
    new.at := current_timestamp;
  end if;
  return new;
end $$;
create function keep_old() returns trigger language plpgsql as $$ begin return old; end $$;
create function maybe_skip() returns trigger language plpgsql as $$ begin if new.name = 'skip' then return null; end if; return new; end $$;
create function skip_in_handler() returns trigger language plpgsql as $$ begin perform 1 / 0; return new; exception when others then return null; end $$;
create function empties_new() returns trigger language plpgsql as $$ begin new := null; return new; end $$;
create function selects_into_new() returns trigger language plpgsql as $$ begin select * into new from stamped where false; return new; end $$;
create function returns_other() returns trigger language plpgsql as $$ declare other record; begin return other; end $$;
create function shadows_new() returns trigger language plpgsql as $$ begin declare new record; begin return new; end; end $$;
create function returns_cast() returns trigger language plpgsql as $$ begin return (null::stamped); end $$;
create function new_or_old() returns trigger language plpgsql as $$ begin if tg_op = 'DELETE' then return old; end if; return new; end $$;
create function no_return() returns trigger language plpgsql as $$ begin new.at := now(); end $$;

create table stamped (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before update on stamped for each row execute function stamp();
create table stamped_all (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on stamped_all for each row execute function stamp();
create table conditional (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before update on conditional for each row execute function stamp_unless_set();
create table skipping (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on skipping for each row execute function maybe_skip();
create table handler (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on handler for each row execute function skip_in_handler();
create table emptied (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on emptied for each row execute function empties_new();
create table selected (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on selected for each row execute function selects_into_new();
create table other (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on other for each row execute function returns_other();
create table shadowed (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on shadowed for each row execute function shadows_new();
create table casted (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert or update on casted for each row execute function returns_cast();
-- OLD is NULL on an insert, so a trigger that returns it skips every insert, and keeps every update.
create table olds (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before insert on olds for each row execute function keep_old();
create table old_on_update (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before update on old_on_update for each row execute function keep_old();
-- NEW is NULL on a delete, so a trigger of both returns NULL there unless it tests which one fired.
create table news_on_delete (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before delete on news_on_delete for each row execute function stamp();
create table both_ways (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before update or delete on both_ways for each row execute function new_or_old();
-- Falling off the end raises, so a trigger with no RETURN skips nothing, and fails every write.
create table no_returns (id int primary key, name text not null, at timestamptz not null default now());
create trigger t before update on no_returns for each row execute function no_return();
-- A trigger on a partition counts, as a row is routed to it.
create table parent (id int, name text not null, at timestamptz not null default now(), primary key (id)) partition by range (id);
create table parent_low partition of parent for values from (0) to (1000);
create trigger t before update on parent_low for each row execute function maybe_skip();
create table stamped_parent (id int, name text not null, at timestamptz not null default now(), primary key (id)) partition by range (id);
create table stamped_parent_low partition of stamped_parent for values from (0) to (1000);
create trigger t before update on stamped_parent_low for each row execute function stamp();
-- SQL and other languages can't be read: `suppress_redundant_updates_trigger` is C.
create table suppressed (id int primary key, name text not null);
create trigger t before update on suppressed for each row execute function suppress_redundant_updates_trigger();
insert into stamped (id, name) values (1, 'a');
insert into stamped_all (id, name) values (1, 'a');
insert into conditional (id, name) values (1, 'a');
insert into old_on_update (id, name) values (1, 'a');
insert into stamped_parent (id, name) values (1, 'a');
insert into suppressed (id, name) values (1, 'a');
