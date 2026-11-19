-- file 1 of 2
alter type region add value 'ap' after 'us';
-- file 2 of 2
create table t_ap partition of t for values in ('ap');
