create type status as enum ('draft', 'active', 'retired');
create type widget_status as enum ('draft', 'active');
create table widgets (id integer primary key, status status not null, history status[]);
