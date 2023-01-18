# Install RPostgres if needed:
#install.packages("RPostgres")
#install.packages("tidyverse")

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
all_updates <- dbGetQuery(con, "
    SELECT *
    FROM analysis.what_did_updates_change
")

all_updates_sub <- all_updates[sample(nrow(all_updates), 100000),] 

all_updates <- all_updates %>% filter(ty != "zero_to_something")

all_updates$ty <- sapply(all_updates$ty, as.character)
all_updates$tyFact <- factor(all_updates$ty, levels=c("bug", "minor", "major"))

head(all_updates)

all_updates <- all_updates %>% mutate(
    did_change_deps = did_add_dep | did_remove_dep | did_modify_dep_constraint,
    only_change_deps = (did_add_dep | did_remove_dep | did_modify_dep_constraint) & !(did_change_types | did_change_code | did_change_json_scripts),
    only_change_types = (did_add_dep | did_remove_dep | did_modify_dep_constraint) & !(did_change_types | did_change_code | did_change_json_scripts)
)

all_updates %>% group_by(tyFact) %>% summarise(
    n = n(), 
    # var_num_did_intro_vuln = sum(did_intro_vuln),
    # var_num_did_patch_vuln = sum(did_patch_vuln),
    var_num_did_change_types = sum(did_change_types),
    var_num_did_change_code = sum(did_change_code),
    var_num_did_add_dep = sum(did_add_dep),
    var_num_did_remove_dep = sum(did_remove_dep),
    var_num_did_modify_dep_constraint = sum(did_modify_dep_constraint),
    var_num_did_change_json_scripts = sum(did_change_json_scripts)
) %>%
mutate(
    # var_pct_did_intro_vuln = var_num_did_intro_vuln / n,
    # var_pct_did_patch_vuln = var_num_did_patch_vuln / n,
    var_pct_did_change_types = var_num_did_change_types / n,
    var_pct_did_change_code = var_num_did_change_code / n,
    var_pct_did_add_dep = var_num_did_add_dep / n,
    var_pct_did_remove_dep = var_num_did_remove_dep / n,
    var_pct_did_modify_dep_constraint = var_num_did_modify_dep_constraint / n,
    var_pct_did_change_json_scripts = var_num_did_change_json_scripts / n
) %>% pivot_longer(
    cols = starts_with("var_pct_"),
    names_to = "var",
    values_to = "val"
) %>% ggplot(aes(x = tyFact, y = val, fill = var)) +
    geom_col(position="dodge") +
    # facet_wrap(~var, scales = "free_y") +
    # theme_minimal() +
    # theme(legend.position = "none") +
    labs(x = "Type of update", y = "Percentage of updates within type", title = "What is changed by each update type?")

#  %>% ggplot(aes(x = tyFact, y = pct, fill = tyFact)) + 
#     geom_bar(stat = "identity") + 
#     facet_wrap(~var, scales = "free_y") + 
#     theme_minimal() + 
#     theme(legend.position = "none") + 
#     labs(x = "Type of update", y = "Proportion of updates", title = "What proportion of updates change each type of thing?")


all_updates %>% summarize(total_did_change_deps=sum(did_change_deps))

as.data.frame(confusionMatrix(factor(all_updates$did_change_deps), factor(all_updates$did_change_code))$table)

plot_change_deps_vs_code <- function(df, update_type) {
        cm <- confusionMatrix(factor(df$did_change_deps), factor(df$did_change_code)) # preds, refs
        cm <- as.data.frame(cm$table)
        # cm$Prediction <- factor(cm$Prediction, levels=rev(levels(cm$Prediction)))
        # cm <- cm %>% mutate(Freq = Freq / sum(Freq))

        ggplot(cm, aes(Prediction,Reference, fill=Freq / sum(Freq))) +
                geom_tile() + geom_text(aes(label=scales::percent(Freq / sum(Freq)))) +
                scale_fill_gradient(low="white", high="#009194") +
                labs(x = "Changed Dependencies", y = "Changed .js / .jsx / .ts / .tsx code") +
                ggtitle(paste("Contents of updates among", update_type, "updates"))
                # scale_x_discrete(labels=c("Class_1","Class_2")) +
                # scale_y_discrete(labels=c("Class_4","Class_3"))

        ggsave(paste("plots/rq4/contents_heat_", update_type, ".pdf", sep=""))
}

plot_change_deps_vs_code(all_updates, "all")
plot_change_deps_vs_code(all_updates %>% filter(ty == "bug"), "bug")
plot_change_deps_vs_code(all_updates %>% filter(ty == "minor"), "minor")
plot_change_deps_vs_code(all_updates %>% filter(ty == "major"), "major")

# creates a data frame with one row per package, and columns for count of each update type
update_changes_by_pkg <- all_updates %>%
    group_by(package_id,tyFact,did_change_deps,did_change_code) %>%
    summarise(
        count = n()
    ) %>% mutate(
        change=ifelse(did_change_deps & did_change_code, "both", ifelse(did_change_deps, "deps", ifelse(did_change_code, "code", "none"))),
    ) %>% pivot_wider(names_from = change, values_from = count, values_fill=0) %>%
    group_by(package_id,tyFact) %>% summarise(
        # total = sum(both, deps, code, none),
        # bothPct = both / total,
        # depsPct = deps / total,
        # codePct = code / total,
        # nonePct = none / total
        total_deps = sum(deps),
        total_code = sum(code),
        total_both = sum(both),
        total_none = sum(none),
    ) %>% mutate(
        total = total_deps + total_code + total_both + total_none,
        bothPct = total_both / total,
        depsPct = total_deps / total,
        codePct = total_code / total,
        nonePct = total_none / total
    ) %>% pivot_longer(
        cols = ends_with("Pct"),
        names_to = "change",
        values_to = "pct"
    ) 
    
    # %>% ggplot(aes(x = tyFact, y = pct, fill = change)) +
    
    # %>% pivot_longer(
    #     cols = starts_with("none"),
    #     names_to = "change",
    #     values_to = "pct"
    # ) %>% ggplot(aes(x = tyFact, y = pct, fill = change)) +
    # mutate(total = bug + minor + major,
    #        bugPct = bug / total,
    #        majorPct = major / total,
    #        minorPct = minor / total,
    # ) %>%
    # mutate(update_action = ifelse(did_intro_vuln, 'Intro vuln', ifelse(did_patch_vuln, 'Patch vuln', 'No security effect')))

update_changes_by_pkg

update_changes_by_pkg$tyFact <- recode(update_changes_by_pkg$tyFact, bug='Bug', minor='Minor', major='Major')


update_changes_by_pkg$change <- recode(update_changes_by_pkg$change, nonePct='Neither', bothPct='Both', depsPct='Dependencies', codePct='.js / .jsx / .ts / .tsx')
update_changes_by_pkg$change <- factor(update_changes_by_pkg$change, levels=c('Neither', 'Dependencies', '.js / .jsx / .ts / .tsx', 'Both'))

# box plots of the percentage of updates that are each type
ggplot(data = update_changes_by_pkg, aes(x = tyFact, y = pct, fill=change)) +
    geom_boxplot() +
    #sets the labels for the x-axis:
    # scale_x_discrete(limits=c("normal", "introduce vuln", "patch vuln")) +
    scale_y_continuous(labels = scales::percent) + 
    #sets the title of the plot
    labs(title = "Percentage of each category of update contents across semver increment types", fill='Contents of Update', x='Semver Increment Type', y = 'Percentage of each packages\' updates')

ggsave("plots/rq4/contents_box_plot.pdf")
ggsave("plots/rq4/contents_box_plot.png")

ggplot(data = update_changes_by_pkg, aes(x = tyFact, y = pct, fill=change)) +
    geom_boxplot() +

    stat_summary(geom="text", fun.y=quantile,
               aes(label=sprintf("%1.4f", ..y..), x=tyFact, color=change),
               position=position_nudge(x=0.45), 
               size=3.5) +

    #sets the labels for the x-axis:
    # scale_x_discrete(limits=c("normal", "introduce vuln", "patch vuln")) +
    scale_y_continuous(labels = scales::percent) + 
    #sets the title of the plot
    labs(title = "Percentage of each category of update contents across semver increment types", fill='Contents of Update', x='Semver Increment Type', y = 'Percentage of each packages\' updates')



