library(DBI)
library(ggplot2)
library(tidyverse)
library(caret)
library(scales)

con <- dbConnect(
    RPostgres::Postgres(),
    dbname = 'npm_data', 
    user = 'data_analyzer',
)

# This takes about 40 seconds to load (when running on the VM!), and takes about 4GB of memory
df_full <- dbGetQuery(con, "
    select update_from_id, update_to_id, downstream_package_id, (unnest(oldnesses)).* from historic_solver_job_results_oldnesses
    where array_length(oldnesses, 1) is not null
")

df_sub <- df_full[sample(nrow(df_full), 10000),] 

df <- df_full

downstream_oldness_stats <- df %>% 
    group_by(downstream_package_id) %>% 
    summarise(
        n = n(),
        mean_old_secs = mean(as.numeric(old_secs)),
        num_old = sum(old_secs > 10),
        perc_old = num_old / n
    ) %>% arrange(desc(n))

head(downstream_oldness_stats)

ggplot(data=downstream_oldness_stats, aes(x=mean_old_secs)) + 
    geom_histogram()


ggplot(data=downstream_oldness_stats, aes(x=perc_old)) + 
    geom_histogram() +
    scale_x_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Number of packages")

ggsave("plots/rq3/perc_old_hist.pdf")

ggplot(data=downstream_oldness_stats, aes(x=perc_old)) + 
    stat_ecdf() +
    scale_x_continuous(labels = scales::percent) +
    scale_y_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Percent of packages")

ggsave("plots/rq3/perc_old_ecdf.pdf")

downstream_oldness_stats %>% summarise(mean(n))

downstream_oldness_stats %>% filter(perc_old == 0) %>% summarise(mean(n))

downstream_oldness_stats %>% filter(perc_old > 0) %>% summarise(mean(n))

downstream_oldness_stats %>% 
# filter(perc_old > 0) %>% 
summarise(mean(as.numeric(mean_old_secs))) / (60 * 60 * 24)

downstream_oldness_stats %>% group_by(round(perc_old * 100)) %>% summarise(mean(n))

ggplot(data=downstream_oldness_stats, aes(x=n)) + 
    geom_histogram() +
    # scale_x_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Number of packages")

ggplot(data=downstream_oldness_stats, aes(x=n, y=perc_old)) + 
    geom_point() #+
    # scale_x_continuous(labels = scales::percent) +
    # scale_y_continuous(labels = scales::percent) +
    # xlab("Percent of out-of-date installed dependencies") +
    # ylab("Percent of packages")

# ggsave("plots/rq3/perc_old_ecdf.pdf")

ggplot(data=downstream_oldness_stats, aes(x=n)) + 
    geom_histogram() +
    scale_x_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Number of packages")


