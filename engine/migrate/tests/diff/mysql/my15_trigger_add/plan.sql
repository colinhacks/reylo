create trigger users_touch before update on users for each row set new.updated_at = now();
