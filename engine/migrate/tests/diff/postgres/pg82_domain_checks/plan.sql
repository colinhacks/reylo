alter domain positive_int add constraint positive_int_check check (value > 0) not valid;
create function is_email(text) returns boolean language sql immutable as $$ select $1 like '%@%' $$;
create domain email as text check (is_email(value));
create table contacts (
  id integer not null,
  address email,
  constraint contacts_pkey primary key (id)
);
