alter table transactions
  drop index transactions_key,
  add key transactions_account_fk (user_id, account_id);
