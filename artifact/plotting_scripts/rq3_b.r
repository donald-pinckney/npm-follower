library(DBI)
library(ggplot2)
library(tidyverse)
library(caret)
library(scales)
library(ggsankey)
library(ggalluvial)

con <- dbConnect(
    RPostgres::Postgres(),
    dbname = 'npm_data', 
    user = 'data_analyzer',
)

# This takes about 40 seconds to load (when running on the VM!), and takes about 4GB of memory
df <- dbGetQuery(con, "
    select 
        instant_update, 
        update_days, 
        downstream_updated_req, 
        is_intro, 
        is_patch, 
        is_neutral, 
        downstream_package_id, 
        result_category,
        ty as update_type 
    from analysis.old_historic_solver_job_flow_info f
    inner join analysis.all_updates u on u.from_id = f.update_from_id and u.to_id = f.update_to_id
")

head(df)

clean_df <- df %>% mutate(
    security_effect = ifelse(is_intro, 'introduces', ifelse(is_patch, 'patches', 'neutral')),
    developer_intervention = ifelse(downstream_updated_req, 'intervention', 'no intervention'),
    resolution = ifelse(result_category == 'Ok' & instant_update, 'instant update', ifelse(result_category == 'Ok', 'delayed update', ifelse(instant_update, 'WEIRD', 'deleted dependency')))
) %>% 
filter(resolution != "WEIRD") %>% # A small number of updates instantly have deleted dependencies. These are anomalies caused by circular dependencies. We ignore them.
filter(!(resolution == "instant update" & developer_intervention == "intervention")) %>% # A small number of updates are themselves the updates, due to circular dependencies. We ignore them.
select(security_effect, update_type, developer_intervention, resolution, update_days)

clean_df$update_type <- factor(as.character(clean_df$update_type), levels=c("bug", "minor", "major"))
clean_df$security_effect <- factor(clean_df$security_effect, levels=c("neutral", "patches", "introduces"))
clean_df$developer_intervention <- factor(clean_df$developer_intervention, levels=c("no intervention", "intervention"))
clean_df$resolution <- factor(clean_df$resolution, levels=c("instant update", "delayed update", "deleted dependency"))

clean_df %>% group_by(developer_intervention, resolution) %>% summarise(count = n())

sec_percs <- clean_df %>% group_by(security_effect) %>% 
    summarise(count = n()) %>%
    mutate(per=paste0(round(count/sum(count)*100, 2), "%")) %>% 
    ungroup()
    
introduces_perc <- sec_percs %>% filter(security_effect == "introduces") %>% pull(per)
patches_perc <- sec_percs %>% filter(security_effect == "patches") %>% pull(per)
neutral_perc <- sec_percs %>% filter(security_effect == "neutral") %>% pull(per)


long_df <- clean_df %>% mutate(sec_effect2 = security_effect) %>% make_long(security_effect, update_type, developer_intervention, resolution, value=sec_effect2)

long_df <- long_df %>% mutate(
    weight=ifelse(value == "introduces", 200, ifelse(value == "patches", 10, 1)),
    sec_effect2 = value
) %>% select(-value)

node_order <- c("neutral", "patches", "introduces", "bug", "minor", "major", "no intervention", "intervention", "instant update", "delayed update", "deleted dependency")
long_df$node <- factor(long_df$node, levels=node_order)
long_df$next_node <- factor(long_df$next_node, levels=node_order)
long_df$sec_effect2 <- factor(long_df$sec_effect2, levels=c("neutral", "patches", "introduces"))


long_df$x <- recode_factor(long_df$x, security_effect = "Security Effect", update_type = "Update Type", developer_intervention = "Developer Intervention", resolution = "Resolution")
long_df$next_x <- recode_factor(long_df$next_x, security_effect = "Security Effect", update_type = "Update Type", developer_intervention = "Developer Intervention", resolution = "Resolution")

levels(long_df$x)

head(long_df)

long_df %>% mutate(
    extra_lab = ifelse(node == "introduces", paste0("\n(", introduces_perc, ")"), ifelse(node == "patches", paste0("\n(", patches_perc, ")"), ifelse(node == "neutral", paste0("\n(", neutral_perc, ")"), "")))
) %>% ggplot( 
    aes(x = x, 
        next_x = next_x, 
        node = node, 
        next_node = next_node,
        fill = factor(node),
        value = weight,
        label = paste0(node, extra_lab))) +
    geom_sankey(flow.alpha = 0.8, node.color = 1) +
    geom_sankey_label(size = 3.5, color = 1, fill = "white") +
    # geom_text()
    scale_fill_viridis_d() +
    theme_sankey(base_size = 16) +
    labs(x = NULL) + 
    theme(legend.position = "none")

ggsave("plots/rq3/flow_analysis.pdf")


alluvial_df <- clean_df %>% group_by(security_effect, update_type, developer_intervention, resolution) %>% summarise(count = n())

head(alluvial_df)

ggplot(alluvial_df,
       aes(y = count, axis1 = security_effect, axis2 = update_type, axis3 = developer_intervention, axis4 = resolution)) +
  geom_alluvium(aes(fill = security_effect), width = 1/12) +
  geom_stratum(width = 1/12, fill = "black", color = "grey") +
  geom_label(stat = "stratum", aes(label = after_stat(stratum))) +
#   scale_x_discrete(limits = c("Gender", "Dept"), expand = c(.05, .05)) +
  scale_fill_brewer(type = "qual", palette = "Set1")

mtcars

mtcars %>% make_long(cyl, vs, am, gear, carb)

df


