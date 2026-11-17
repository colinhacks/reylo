create table prices (
  id int not null primary key,
  amount decimal(12, 2) not null,
  note nvarchar(100) null,
  valid_from datetime2(7) generated always as row start hidden not null,
  valid_to datetime2(7) generated always as row end hidden not null,
  period for system_time (valid_from, valid_to)
) with (system_versioning = on (history_table = dbo.prices_history));
