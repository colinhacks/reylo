create table d (id integer, meta struct(a integer), primary key (id), check ((meta.a > 0)));
