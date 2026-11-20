create table p (id int not null, c date) partition by list (id);
create table x (id int not null, c date);
