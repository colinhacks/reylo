drop index ix;
create index ix on t(qty, n);
comment on index ix is 'by qty';
