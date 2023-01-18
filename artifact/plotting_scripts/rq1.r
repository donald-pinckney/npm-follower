# Install RPostgres if needed:
#install.packages("RPostgres")
#install.packages("tidyverse")

library(DBI)
library(ggplot2)
library(tidyverse)

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
    theme_minimal() +
    # theme(legend.position="bottom") +
    labs(x="Year", y="Percentage of dependencies", fill="Constraint type")

ggsave("plots/rq1/constraint_usage_over_time.pdf")



dep_by_year_percs

ggplot(data = head(unique_deps_across_versions, 1000), aes(x = year)) +
    geom_area(aes(fill = composite_constraint_typeFact), position="fill") +
    scale_x_discrete()

# Takes about 20 seconds
ggplot(data = unique_deps_across_versions, aes(x = composite_constraint_typeFact, fill=dep_type)) +
    geom_bar()

# creates a data frame with one row per package, and columns for count of each update type
# constraintCountsByPackage <- 
total_unqiue_deps <- unique_deps_across_versions %>%
    group_by(package_id,composite_constraint_typeFact) %>%
    summarise(
        count = n()
    ) %>%
    group_by(package_id) %>%
    summarise(
        total = sum(count),
    )


head(total_unqiue_deps)

constraintCountsByPackageAndDepType <- unique_deps_across_versions %>%
    group_by(package_id,composite_constraint_typeFact) %>%
    summarise(
        count = n()
    ) %>%
    inner_join(total_unqiue_deps, by=c('package_id')) %>%
    mutate(pct = count / total) %>%
    pivot_wider(names_from = composite_constraint_typeFact, values_from = c(count, pct), values_fill=0)


# box plots of the percentage of updates that are each type
ggplot(data = constraintCountsByPackageAndDepType %>% pivot_longer(cols=starts_with("pct"), names_to="constraint_type", values_to="val"), aes(x = constraint_type, y = val)) +
    geom_boxplot() +
    #sets the labels for the x-axis:
    # scale_x_discrete(labels=c("bug", "major", "minor", "zero")) +
    # scale_y_continuous(labels = scales::percent) + 
    #sets the title of the plot
    labs(title = "Percentage of updates that are each type")




