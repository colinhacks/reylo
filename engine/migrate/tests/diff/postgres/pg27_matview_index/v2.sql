create table sales (id integer primary key, region text not null, amount numeric not null);
create materialized view region_totals as select region, sum(amount) as total, count(*) as n from sales group by region;
create unique index region_totals_region on region_totals (region);
create index region_totals_total on region_totals (total desc);
