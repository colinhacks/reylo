create table a (id integer not null, name text not null, b_email text not null, constraint a_b_email_fkey foreign key (b_email) references b (email));
create table b (id integer not null, email text not null, a_name text not null, constraint b_a_name_fkey foreign key (a_name) references a (name));
create unique index a_name_key on a (name);
create unique index b_email_key on b (email);
