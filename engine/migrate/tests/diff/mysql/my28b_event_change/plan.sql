drop event prune;
create event prune on schedule every 1 hour comment 'hourly' do delete from log where at < now() - interval 7 day;
