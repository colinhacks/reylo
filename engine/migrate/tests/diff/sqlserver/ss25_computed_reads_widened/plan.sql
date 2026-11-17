drop index lines_total on lines;
alter table lines drop column label;
alter table lines drop column total;
alter table lines alter column qty bigint not null;
alter table lines add label as (concat(N'#',[qty]));
alter table lines add total as ([qty]*[price]) persisted;
create nonclustered index lines_total on lines (total);
