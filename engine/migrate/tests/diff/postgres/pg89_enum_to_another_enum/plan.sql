alter table widgets alter column history type widget_status[] using history::text[]::widget_status[];
alter table widgets alter column status type widget_status using status::text::widget_status;
