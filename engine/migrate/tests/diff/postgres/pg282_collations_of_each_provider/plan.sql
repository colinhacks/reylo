create collation ctest_det (provider = icu, locale = 'und');
create collation ctest_nondet (provider = icu, locale = 'und', deterministic = false);
create collation plain_builtin (provider = builtin, locale = 'C.UTF-8');
create table t (
  a text collate ctest_nondet,
  b text collate plain_builtin
);
