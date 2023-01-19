# Install RPostgres if needed:
#install.packages("RPostgres")
#install.packages("tidyverse")

library(DBI)
library(ggplot2)
library(tidyverse)

mytheme <- function() {
  return(theme_minimal() +
           theme(
             # NOTE: UNCOMMENT WHEN RENDING PLOTS FOR THE PAPER
             # (can't get the CM fonts to work in artifact VM...)
             text = element_text(family = "Times", size=10),
              # panel.grid.major = element_blank(),
             # panel.grid.minor = element_blank(),
             # panel.grid.major = element_line(colour="gray", size=0.1),
             # panel.grid.minor =
             #  element_line(colour="gray", size=0.1, linetype='dotted'),
            #  axis.ticks = element_line(size=0.05),
            #  axis.ticks.length=unit("-0.05", "in"),
            #  axis.text.y = element_text(margin = margin(r = 5)),
             axis.text.x = element_text(family = "Times", size=5),
             legend.key = element_rect(colour=NA),
             legend.spacing = unit(0.001, "in"),
             legend.key.size = unit(0.2, "in"),
            #  legend.title = element_blank(),
            #  legend.position = c(0.75, .7),
             legend.background = element_blank()))
}

mysave <- function(filename) {
  ggsave(filename, width=4, height=3, units=c("in"))
}


con <- dbConnect(
    RPostgres::Postgres(),
    dbname = 'npm_data', 
    user = 'data_analyzer',
)

# This takes about 40 seconds to load (when running on the VM!), and takes about 4GB of memory
unique_deps_across_versions <- dbGetQuery(con, "
    SELECT *
    FROM analysis.unique_deps_yearly_latest --unique_deps_yearly_latest_depended_on_only, unique_deps_yearly_latest
")

unique_deps_across_versions$dep_type <- sapply(unique_deps_across_versions$dep_type, as.character)
unique_deps_across_versions$dep_typeFact <- as.factor(unique_deps_across_versions$dep_type)
unique_deps_across_versions$composite_constraint_type <- sapply(unique_deps_across_versions$composite_constraint_type, as.character)
unique_deps_across_versions$composite_constraint_typeFact <- as.factor(unique_deps_across_versions$composite_constraint_type)

unique_deps_across_versions$composite_constraint_typeFact <- fct_collapse(unique_deps_across_versions$composite_constraint_typeFact,
    "Minor (^1.2.3)"="range-minor",
    "Exact (=1.2.3)"="range-=",
    "Bug (~1.2.3)"="range-patch",
    "Any (*)"="range-major",
    "Geq (>=1.2.3)"="range->=",
    # Tag="tag",
    other_level="Other"
)

unique_deps_across_versions$composite_constraint_typeFact <- factor(
    unique_deps_across_versions$composite_constraint_typeFact, levels=c("Exact (=1.2.3)", "Bug (~1.2.3)", "Minor (^1.2.3)", "Geq (>=1.2.3)", "Any (*)", "Other"))

head(unique_deps_across_versions)

year_sums <- unique_deps_across_versions %>% 
    group_by(year) %>% 
    summarise(year_total=n())

# Get the percentage of each composite_constraint_typeFact within each year



dep_by_year_percs <- unique_deps_across_versions %>% 
    group_by(year, composite_constraint_typeFact) %>% 
    summarise(count=n()) %>% 
    inner_join(year_sums, by="year") %>%
    mutate(percentage = count / year_total)
    # mutate(percentage = count / sum(count)) 
    # %>% 
    # ggplot(aes(x=yearFact, y=percentage, fill=composite_constraint_typeFact)) +
    #     geom_bar(stat="identity") +
    #     theme(axis.text.x = element_text(angle = 90, hjust = 1)) +
    #     labs(x="Year", y="Percentage of Dependencies", fill="Constraint Type") +
    #     scale_fill_brewer(palette="Set1") +
    #     theme(legend.position="bottom")

head(dep_by_year_percs)

dep_by_year_percs$yearFact <- factor(dep_by_year_percs$year)

# plot a stacked area plot of composite_constraint_typeFact over year
ggplot(dep_by_year_percs, aes(x=yearFact, y=percentage, fill=composite_constraint_typeFact)) +
    geom_col() +
    scale_x_discrete() +
    scale_fill_brewer(palette="Set1") +
    scale_y_continuous(labels = scales::percent) + 
    # theme_minimal() +
    # theme(legend.position="bottom") +
    labs(x="Year", y="Percentage of dependencies", fill="Constraint type") +
    mytheme()

mysave("plots/rq1/constraint_usage_over_time.pdf")

dep_by_year_percs


