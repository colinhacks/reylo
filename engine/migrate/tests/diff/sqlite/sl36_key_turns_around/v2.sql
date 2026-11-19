create table owner (id integer primary key autoincrement, ownerid integer not null, constraint owner_ownerid_fkey foreign key (ownerid) references cat (id) on delete restrict on update cascade);
create table cat (id integer primary key autoincrement);
create index ownerid on owner (ownerid);
