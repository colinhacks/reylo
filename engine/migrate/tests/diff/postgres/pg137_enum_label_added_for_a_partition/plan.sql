alter type region add value 'ap' after 'us';
commit;
begin;
create table t_ap partition of t for values in ('ap');
