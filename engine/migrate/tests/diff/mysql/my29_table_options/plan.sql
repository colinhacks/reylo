alter table users
  add column tag varchar(10) collate utf8mb4_unicode_ci,
  collate=utf8mb4_unicode_ci,
  engine=MyISAM;
