alter table lines add label as (concat(N'#',[id]));
alter table lines add total as ([qty]*[price]) persisted;
