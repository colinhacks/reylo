create table p (id int, d date not null, data text) partition by range (d);
create table p1 partition of p for values from ('2027-01-01') to ('2028-01-01');
