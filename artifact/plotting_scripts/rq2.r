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
all_updates <- dbGetQuery(con, "
    SELECT 
        package_id, 
        from_id, 
        to_id, 
        (from_semver).major AS from_semver_major, 
        (from_semver).minor AS from_semver_minor, 
        (from_semver).bug AS from_semver_bug,
        (to_semver).major AS to_semver_major, 
        (to_semver).minor AS to_semver_minor, 
        (to_semver).bug AS to_semver_bug,
        from_created,
        to_created,
        ty,
        ROW(from_id, to_id) IN (SELECT from_id, to_id FROM analysis.vuln_intro_updates) as did_intro_vuln,
        ROW(from_id, to_id) IN (SELECT from_id, to_id FROM analysis.vuln_patch_updates) as did_patch_vuln
    FROM analysis.all_updates
")

all_updates_sub <- all_updates[sample(nrow(all_updates), 100000),] 

all_updates <- all_updates %>% filter(ty != "zero_to_something")

all_updates$ty <- sapply(all_updates$ty, as.character)
all_updates$tyFact <- factor(all_updates$ty, levels=c("bug", "minor", "major"))

head(all_updates)

patches_only <- all_updates %>% filter(did_patch_vuln == TRUE & did_intro_vuln == FALSE)
others <- all_updates %>% filter(did_patch_vuln == FALSE | did_intro_vuln == TRUE)

# select the row with ty ordered bug,minor,major for each from_id
patches_lowest_type <- patches_only %>% group_by(from_id) %>% top_n(-1, tyFact)
patches_first_created <- patches_only %>% group_by(from_id) %>% top_n(-1, to_created)

patches_only

# creates a data frame with one row per package, and columns for count of each update type
updateCountsByPackage <- all_updates %>%
    group_by(package_id,did_intro_vuln,did_patch_vuln,tyFact) %>%
    summarise(
        count = n()
    ) %>% 
    pivot_wider(names_from = tyFact, values_from = count, values_fill=0) %>%
    mutate(total = bug + minor + major,
           bugPct = bug / total,
           majorPct = major / total,
           minorPct = minor / total,
    ) %>%
    mutate(update_action = ifelse(did_intro_vuln, 'Intro vuln', ifelse(did_patch_vuln, 'Patch vuln', 'No security effect')))

head(updateCountsByPackage)

# make a long version of it for ggplot
updateCountsByPackageLong <- updateCountsByPackage %>%
    pivot_longer(cols=ends_with("Pct"), names_to="ty", values_to="pct")

updateCountsByPackageLong$update_action <- factor(updateCountsByPackageLong$update_action, levels=c('No security effect', 'Intro vuln', 'Patch vuln'))
updateCountsByPackageLong$ty <- recode(updateCountsByPackageLong$ty, bugPct='Bug', minorPct='Minor', majorPct='Major')
updateCountsByPackageLong$ty <- factor(updateCountsByPackageLong$ty, levels=c('Bug', 'Minor', 'Major'))

head(updateCountsByPackageLong)

# box plots of the percentage of updates that are each type
ggplot(data = updateCountsByPackageLong, aes(x = update_action, y = pct, fill=ty)) +
    geom_boxplot() +
    #sets the labels for the x-axis:
    # scale_x_discrete(limits=c("normal", "introduce vuln", "patch vuln")) +
    scale_y_continuous(labels = scales::percent) + 
    #sets the title of the plot
    labs(title = "Percentage of each semver increment type across security effects", fill='Semver Increment Type', x='Update Security Effect', y = 'Percentage of each package\'s updates')

ggsave("plots/rq2/update_type_with_security.pdf")
ggsave("plots/rq2/update_type_with_security.png")


