create table owner (id integer primary key autoincrement);
create table cat (id integer primary key autoincrement, ownerid integer not null, constraint cat_ownerid_fkey foreign key (ownerid) references owner (id) on delete restrict on update cascade);
create table other (id integer primary key autoincrement, ownerid integer not null);
create index cat_ownerid_idx on cat (ownerid);
create index ownerid on other (ownerid);
