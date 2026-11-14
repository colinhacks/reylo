create table ev (id integer primary key, tags varchar[], meta struct(a integer, b varchar, c boolean), extra map(varchar, integer));
