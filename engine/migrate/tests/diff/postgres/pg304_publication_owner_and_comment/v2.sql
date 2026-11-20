create table t (id int);
create publication pub for table t;
alter publication pub owner to diff_pub_owner;
comment on publication pub is 'all of t';
