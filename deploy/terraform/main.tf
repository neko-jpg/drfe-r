# DRFE-R Multi-Region Deployment
# Terraform configuration for global testbed

terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

# Variables
variable "regions" {
  description = "AWS regions for deployment"
  type        = list(string)
  default     = [
    "us-east-1",
    "us-west-2",
    "eu-west-1",
    "eu-central-1",
    "ap-southeast-1",
    "ap-northeast-1",
    "ap-south-1",
    "sa-east-1"
  ]
}

variable "nodes_per_region" {
  description = "Number of DRFE-R nodes per region"
  type        = number
  default     = 5
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
  default     = "t3.micro"
}

# Provider for each region
provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

provider "aws" {
  alias  = "us_west_2"
  region = "us-west-2"
}

provider "aws" {
  alias  = "eu_west_1"
  region = "eu-west-1"
}

provider "aws" {
  alias  = "ap_northeast_1"
  region = "ap-northeast-1"
}

# Security Group (per region)
resource "aws_security_group" "drfer_sg" {
  provider    = aws.us_east_1
  name        = "drfer-security-group"
  description = "Security group for DRFE-R nodes"

  # DRFE-R UDP port
  ingress {
    from_port   = 8080
    to_port     = 8090
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # DRFE-R TCP port
  ingress {
    from_port   = 8080
    to_port     = 8090
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # SSH
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Prometheus metrics
  ingress {
    from_port   = 9090
    to_port     = 9090
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name    = "drfer-sg"
    Project = "DRFE-R"
  }
}

# EC2 Instance Module
module "drfer_nodes_us_east" {
  source = "./modules/drfer_node"
  
  providers = {
    aws = aws.us_east_1
  }
  
  region           = "us-east-1"
  node_count       = var.nodes_per_region
  instance_type    = var.instance_type
  security_group_id = aws_security_group.drfer_sg.id
}

# Output
output "node_ips" {
  description = "Map of region to node IPs"
  value = {
    us_east_1 = module.drfer_nodes_us_east.node_ips
  }
}

output "total_nodes" {
  description = "Total number of deployed nodes"
  value       = var.nodes_per_region * length(var.regions)
}
