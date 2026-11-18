alter table users drop foreign key users_ibfk_1, rename index tid to tid;
alter table users add constraint users_ibfk_1 foreign key (tid) references some_table (id) on delete cascade;
