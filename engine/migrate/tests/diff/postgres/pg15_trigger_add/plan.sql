create trigger users_touch before update on users for each row execute function touch();
