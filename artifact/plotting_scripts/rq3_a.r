library(DBI)
library(ggplot2)
library(tidyverse)
library(caret)
library(scales)

mytheme <- function() {
  return(theme_bw() +
           theme(
             # NOTE: UNCOMMENT WHEN RENDING PLOTS FOR THE PAPER
             # (can't get the CM fonts to work in artifact VM...)
             text = element_text(family = "Times", size=10),
              panel.grid.major = element_blank(),
             # panel.grid.minor = element_blank(),
             # panel.grid.major = element_line(colour="gray", size=0.1),
             # panel.grid.minor =
             #  element_line(colour="gray", size=0.1, linetype='dotted'),
             axis.ticks = element_line(size=0.05),
             axis.ticks.length=unit("-0.05", "in"),
             axis.text.y = element_text(margin = margin(r = 5)),
             axis.text.x = element_text(hjust=1),
             legend.key = element_rect(colour=NA),
             legend.spacing = unit(0.001, "in"),
             legend.key.size = unit(0.2, "in"),
             legend.title = element_blank(),
             legend.position = c(0.75, .7),
             legend.background = element_blank()))
}

mysave <- function(filename) {
  ggsave(filename, width=4, height=3, units=c("in"))
  # embed_font(path)
}

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


ggplot(data=downstream_oldness_stats, aes(x=perc_old)) + 
    geom_histogram() +
    scale_x_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Number of packages") +
    mytheme()

mysave("plots/rq3/perc_old_hist.pdf")

ggplot(data=downstream_oldness_stats, aes(x=perc_old)) + 
    stat_ecdf() +
    scale_x_continuous(labels = scales::percent) +
    scale_y_continuous(labels = scales::percent) +
    xlab("Percent of out-of-date installed dependencies") +
    ylab("Cumulative percent of packages") +
    mytheme()

mysave("plots/rq3/perc_old_ecdf.pdf")

100 * (downstream_oldness_stats %>% filter(perc_old >= 0.25) %>% nrow()) / nrow(downstream_oldness_stats)

100 * (downstream_oldness_stats %>% filter(perc_old <= 0) %>% nrow()) / nrow(downstream_oldness_stats)

downstream_oldness_stats %>% summarise(mean(n))

downstream_oldness_stats %>% filter(perc_old <= 0) %>% summarise(mean(n))

downstream_oldness_stats %>% filter(perc_old > 0) %>% summarise(mean(n))

downstream_oldness_stats %>% 
summarise(mean(as.numeric(mean_old_secs))) / (60 * 60 * 24)

downstream_oldness_stats %>% 
filter(perc_old > 0) %>% 
summarise(mean(as.numeric(mean_old_secs))) / (60 * 60 * 24)


