select table_comment, create_options, create_time
from information_schema.tables
where table_schema = database()
