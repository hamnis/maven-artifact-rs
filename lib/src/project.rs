use crate::artifact::{Artifact, ParseArtifactError, PartialArtifact};
use crate::{ArtifactId, GroupId, Version};
use std::collections::HashMap;
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct Dependency {
    pub artifact: Artifact,
    pub scope: Option<String>,
}

impl Dependency {
    pub fn new(artifact: Artifact) -> Dependency {
        Dependency {
            artifact,
            scope: None,
        }
    }

    pub fn resolve_properties(&self, props: &HashMap<String, String>) -> Dependency {
        Dependency {
            artifact: self.artifact.resolve_properties(props),
            scope: self.scope.clone(),
        }
    }

    pub fn mngt_key(&self) -> String {
        let partial = PartialArtifact::from(self.artifact.clone());
        partial.to_string()
    }

    pub fn mapped(vec: &[Dependency]) -> HashMap<String, Dependency> {
        vec.iter()
            .map(|dep| (dep.mngt_key(), dep.clone()))
            .collect()
    }

    pub fn is_scope(&self, query: &str) -> bool {
        self.scope.as_ref().is_some_and(|s| s == query)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DependencyManagement {
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub artifact: Artifact,
    pub parent: Option<Artifact>,
    pub dependency_management: DependencyManagement,
    pub dependencies: Vec<Dependency>,
    pub properties: HashMap<String, String>,
}

impl Project {
    pub fn new(artifact: Artifact) -> Self {
        Project {
            artifact: artifact.clone(),
            parent: Option::default(),
            dependency_management: DependencyManagement::default(),
            dependencies: Vec::default(),
            properties: {
                let mut map = HashMap::default();
                map.insert("project.groupId".to_string(), artifact.group_id.to_string());
                map.insert(
                    "project.artifactId".to_string(),
                    artifact.artifact_id.to_string(),
                );
                if let Some(v) = &artifact.version {
                    map.insert("project.version".to_string(), v.to_string());
                }
                map
            },
        }
    }

    pub fn reference(&self) -> ProjectReference {
        ProjectReference::from(&self.artifact)
    }

    pub fn resolve_properties_this(&self) -> Project {
        self.resolve_properties(&self.properties)
    }

    pub fn resolve_properties(&self, properties: &HashMap<String, String>) -> Project {
        let mut modified = self.clone();
        modified.artifact = self.artifact.resolve_properties(properties);
        if let Some(parent) = self.parent.clone() {
            modified.parent = Some(parent.resolve_properties(properties))
        }
        modified.dependency_management.dependencies = self
            .dependency_management
            .dependencies
            .iter()
            .map(|d| d.resolve_properties(properties))
            .collect();
        modified.dependencies = self
            .dependencies
            .iter()
            .map(|d| d.resolve_properties(properties))
            .collect();

        modified
    }

    pub fn parse<R: Read + Seek>(input: R) -> Result<Project, crate::pom::PomParserError> {
        crate::pom::PomParser::parse(input)
    }
}

pub struct ProjectReference(Artifact);

impl From<&Artifact> for ProjectReference {
    fn from(value: &Artifact) -> Self {
        ProjectReference(value.clone())
    }
}

impl ProjectReference {
    pub fn new(group_id: GroupId, artifact_id: ArtifactId, version: Version) -> ProjectReference {
        ProjectReference(Artifact::new(group_id, artifact_id, version))
    }

    pub fn parse(input: &str) -> Result<ProjectReference, ParseArtifactError> {
        let parts: Vec<_> = input.split(":").collect();
        if parts.len() == 3 {
            Ok(Self::new(
                GroupId::from(parts[0]),
                ArtifactId::from(parts[1]),
                Version::from(parts[2]),
            ))
        } else {
            Err(ParseArtifactError::new(format!(
                "There are not enough or too many parts. Expected <groupId>:<artifactId>:<version>, but was {}",
                input
            )))
        }
    }

    pub fn path(&self) -> String {
        self.0.path()
    }

    pub fn pom_file_name(&self) -> String {
        self.0
            .with_extension("pom".to_string())
            .file_name()
            .to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parse_full() {
        let pom = r###"
            <project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
      <modelVersion>4.0.0</modelVersion>
      <groupId>com.mycompany.app</groupId>
      <artifactId>my-app</artifactId>
      <version>1.0-SNAPSHOT</version>
      <name>my-app</name>
      <!-- FIXME change it to the project's website -->
      <url>http://www.example.com</url>
      <properties>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <maven.compiler.release>17</maven.compiler.release>
      </properties>
      <dependencyManagement>
        <dependencies>
          <dependency>
            <groupId>org.junit</groupId>
            <artifactId>junit-bom</artifactId>
            <version>5.11.0</version>
            <type>pom</type>
            <scope>import</scope>
          </dependency>
        </dependencies>
      </dependencyManagement>
      <dependencies>
        <dependency>
          <groupId>org.junit.jupiter</groupId>
          <artifactId>junit-jupiter-api</artifactId>
          <scope>test</scope>
        </dependency>
        <!-- Optionally: parameterized tests support -->
        <dependency>
          <groupId>org.junit.jupiter</groupId>
          <artifactId>junit-jupiter-params</artifactId>
          <scope>test</scope>
        </dependency>
      </dependencies>
      <build>
        <pluginManagement><!-- lock down plugins versions to avoid using Maven defaults (may be moved to parent pom) -->
           ... lots of helpful plugins
        </pluginManagement>
      </build>
    </project>
        "###;

        let parsed = Project::parse(Cursor::new(pom));
        println!("{:?}", parsed);
        assert!(parsed.is_ok());
    }

    #[test]
    fn resolve_properties() {
        let mut project = Project::new(Artifact::new(
            GroupId::from("com.example"),
            ArtifactId::from("example"),
            Version::from("1.2.3"),
        ));
        let dep =
            Dependency::new(Artifact::parse("com.example:example-lib:${project.version}").unwrap());
        project.dependencies.push(dep.clone());

        let resolved = project.resolve_properties_this();
        assert_eq!(
            &project.artifact.version,
            &resolved.dependencies.first().unwrap().artifact.version
        );

        assert_ne!(
            &dep.artifact.version,
            &resolved.dependencies.first().unwrap().artifact.version
        );
    }
}
